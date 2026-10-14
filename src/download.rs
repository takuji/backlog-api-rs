use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use futures_util::StreamExt;
use tokio::io::AsyncWriteExt;

use crate::origin_mark::{self, OriginMark};
use crate::{Error, Result};

/// `path` 用の一意な一時ファイルパスを作る(`{path}.{pid}-{n}.part`)。
///
/// プロセスIDと単調増加するカウンタを組み合わせることで、同じ宛先への
/// 同時ダウンロードが同じ一時ファイルを取り合って壊さないようにする。
/// ただし SIGKILL のようなハードキルではこの後の [`TempFileGuard`] の
/// `Drop` すら実行されないため、その場合は一時ファイルが残り得る。
pub(crate) fn temp_path_for(path: &Path) -> PathBuf {
    static NEXT: AtomicU64 = AtomicU64::new(0);
    let n = NEXT.fetch_add(1, Ordering::Relaxed);
    let pid = std::process::id();
    let mut temp_path = path.to_path_buf().into_os_string();
    temp_path.push(format!(".{pid}-{n}.part"));
    PathBuf::from(temp_path)
}

/// 一時ファイルをスコープ終了時に削除するガード。
///
/// エラー時だけでなく、Future がキャンセルされて drop されたときにも
/// `.part` ファイルを残さないために使う。リネームが成功したら
/// [`Self::disarm`] を呼んで削除を取り消す。
struct TempFileGuard {
    path: Option<PathBuf>,
}

impl TempFileGuard {
    fn new(path: PathBuf) -> Self {
        Self { path: Some(path) }
    }

    fn disarm(&mut self) {
        self.path = None;
    }
}

impl Drop for TempFileGuard {
    fn drop(&mut self) {
        // Drop は async にできないため同期APIで削除する
        if let Some(path) = self.path.take() {
            let _ = std::fs::remove_file(path);
        }
    }
}

/// [`AttachmentDownload::save_to`] の結果。
#[derive(Debug)]
pub struct SavedFile {
    /// 書き込んだバイト数。
    pub bytes: u64,
    /// インターネット由来の印を付けられたか。
    ///
    /// 付けられなかった場合もファイルは保存されている。呼び出し側は
    /// ダウンロードを成功として扱い、この値を利用者に報告する。
    pub origin_mark: OriginMark,
}

/// 進行中のダウンロード。ヘッダーは受信済みで、ボディはまだ読んでいない。
///
/// 書き込み先はファイル名(`Content-Disposition`)を見てから決めたいので、
/// ヘッダーの受信とボディの読み出しを分けている。
pub struct AttachmentDownload {
    /// レスポンスの `Content-Type`。
    pub content_type: Option<String>,
    /// `Content-Disposition` から取り出したファイル名。
    pub file_name: Option<String>,
    /// `Content-Length`(宣言があれば)。転送前のサイズ判定に使う。
    pub content_length: Option<u64>,
    response: reqwest::Response,
}

impl std::fmt::Debug for AttachmentDownload {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AttachmentDownload")
            .field("content_type", &self.content_type)
            .field("file_name", &self.file_name)
            .field("content_length", &self.content_length)
            .finish_non_exhaustive()
    }
}

impl AttachmentDownload {
    pub(crate) fn new(response: reqwest::Response) -> Self {
        let content_type = response
            .headers()
            .get(reqwest::header::CONTENT_TYPE)
            .and_then(|v| v.to_str().ok())
            .map(str::to_string);
        let file_name = response
            .headers()
            .get(reqwest::header::CONTENT_DISPOSITION)
            .and_then(|v| v.to_str().ok())
            .and_then(parse_content_disposition_filename);
        let content_length = response.content_length();
        Self {
            content_type,
            file_name,
            content_length,
            response,
        }
    }

    /// ボディをメモリに読み込んで返す。
    ///
    /// アイコンやスペースロゴのような**小さい画像**のための入口。添付ファイルの
    /// ように大きくなりうるものは [`Self::save_to`] を使い、メモリに載せないこと。
    ///
    /// `max_bytes` を超える場合は [`Error::TooLarge`] を返す。`save_to` と同じく、
    /// `Content-Length` が宣言されていれば転送前に、無い場合や偽っている場合は
    /// 受信中の累積バイト数で判定する。
    pub async fn read_to_vec(self, max_bytes: Option<u64>) -> Result<Vec<u8>> {
        if let (Some(limit), Some(declared)) = (max_bytes, self.content_length)
            && declared > limit
        {
            return Err(Error::TooLarge {
                size: declared,
                limit,
            });
        }

        let mut buf: Vec<u8> = Vec::new();
        let mut stream = self.response.bytes_stream();
        while let Some(chunk) = stream.next().await {
            let chunk = chunk.map_err(Error::from)?;
            if let Some(limit) = max_bytes
                && (buf.len() + chunk.len()) as u64 > limit
            {
                return Err(Error::TooLarge {
                    size: (buf.len() + chunk.len()) as u64,
                    limit,
                });
            }
            buf.extend_from_slice(&chunk);
        }
        Ok(buf)
    }

    /// ボディを `path` に書き出す。
    ///
    /// メモリに全体を載せず、受信しながら書く。`path` と同じディレクトリの
    /// 一時ファイル(`{ファイル名}.{pid}-{n}.part`、同時ダウンロードでも
    /// 衝突しないよう一意な名前を使う)に書き、成功したときだけ `path` に
    /// リネームする。失敗・`max_bytes` 超過・Future のキャンセル(MCPクライアントの
    /// 切断など)のいずれでも一時ファイルは [`TempFileGuard`] により削除され、
    /// `path` は作らない。
    ///
    /// `max_bytes` が指定されていて `Content-Length` がそれを超えている場合は、
    /// 転送を始める前に [`Error::TooLarge`] を返す。ヘッダーが無い・正しくない場合も
    /// 受信量を数えて超過時点で中断する。
    ///
    /// 書き込み後、リネームの**前**に「インターネット由来」の印を付ける
    /// ([`crate::OriginMark`])。こうするとファイルは最終パスに現れた瞬間から
    /// 印が付いており、無印のファイルが利用者に見える瞬間がない。
    /// 印を付けられなくてもエラーにはせず、結果を [`SavedFile`] で返す。
    pub async fn save_to(self, path: &Path, max_bytes: Option<u64>) -> Result<SavedFile> {
        if let (Some(limit), Some(declared)) = (max_bytes, self.content_length)
            && declared > limit
        {
            return Err(Error::TooLarge {
                size: declared,
                limit,
            });
        }

        let temp_path = temp_path_for(path);
        let mut guard = TempFileGuard::new(temp_path.clone());

        let mut file = tokio::fs::File::create(&temp_path).await?;
        let mut written: u64 = 0;
        let mut stream = self.response.bytes_stream();

        while let Some(chunk) = stream.next().await {
            let chunk = chunk.map_err(Error::from)?;
            written += chunk.len() as u64;
            if let Some(limit) = max_bytes
                && written > limit
            {
                return Err(Error::TooLarge {
                    size: written,
                    limit,
                });
            }
            file.write_all(&chunk).await?;
        }

        file.flush().await?;
        drop(file);

        // リネームの前に印を付ける。拡張属性・代替データストリームは
        // 同一ボリューム内のリネームで保持される(macOS/APFS で実測確認済み)。
        // 処理量が小さく有界(macOS は setxattr 1回、Windows は代替データ
        // ストリームへの CreateFile+WriteFile+CloseHandle 1回)なので
        // spawn_blocking には載せない(`TempFileGuard::drop` が同期の
        // remove_file を使うのと同じ判断)。
        let origin_mark = origin_mark::apply(&temp_path);

        tokio::fs::rename(&temp_path, path).await?;
        guard.disarm();
        Ok(SavedFile {
            bytes: written,
            origin_mark,
        })
    }
}

/// `Content-Disposition` ヘッダーからファイル名を取り出す。
///
/// `filename*=UTF-8''...`(RFC 5987、日本語ファイル名で使われる)を優先し、
/// 無ければ `filename="..."` を使う。
pub(crate) fn parse_content_disposition_filename(value: &str) -> Option<String> {
    // filename*=UTF-8''%E5%9B%B3.png 形式(percent-encodeされたUTF-8)
    if let Some(start) = value.find("filename*=") {
        let rest = &value[start + "filename*=".len()..];
        let token = rest.split(';').next().unwrap_or(rest).trim();
        if let Some(encoded) = token.split("''").nth(1)
            && let Some(decoded) = percent_decode_utf8(encoded)
            && !decoded.is_empty()
        {
            return Some(decoded);
        }
    }
    // filename="spec.pdf" / filename=spec.pdf 形式
    let start = value.find("filename=")?;
    let rest = &value[start + "filename=".len()..];
    let token = rest.split(';').next().unwrap_or(rest).trim();
    let name = token.trim_matches('"').trim();
    if name.is_empty() {
        None
    } else {
        Some(name.to_string())
    }
}

/// percent-encoding された UTF-8 文字列をデコードする。
///
/// `%` の後に2桁の16進数が続かない場合は、その `%` をリテラルとして扱う。
/// RFC 5987 に違反したヘッダー(`100%.txt` のようにパーセントを
/// エンコードし忘れたもの)でもファイル名を失わないため。位置によって
/// 寛容・厳格が変わらないよう、壊れたエスケープは一貫してリテラル扱いにする。
///
/// デコードしたバイト列が UTF-8 として不正な場合のみ `None` を返す。
fn percent_decode_utf8(input: &str) -> Option<String> {
    let bytes = input.as_bytes();
    let mut out: Vec<u8> = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        match decode_escape(bytes, i) {
            Some(byte) => {
                out.push(byte);
                i += 3;
            }
            None => {
                out.push(bytes[i]);
                i += 1;
            }
        }
    }
    String::from_utf8(out).ok()
}

/// `bytes[i]` から始まる `%XX` を1バイトにデコードする。
///
/// `%` で始まっていない、桁が足りない、16進数でない、のいずれかなら `None`。
/// 呼び出し側はその場合その1バイトをそのまま出力する。
fn decode_escape(bytes: &[u8], i: usize) -> Option<u8> {
    if bytes[i] != b'%' {
        return None;
    }
    let hex = bytes.get(i + 1..i + 3)?;
    // `u8::from_str_radix` は "+4" のような符号付き表記を受け付けてしまうため、
    // 2桁とも16進数であることを先に確かめる。
    if !hex.iter().all(u8::is_ascii_hexdigit) {
        return None;
    }
    let hex = std::str::from_utf8(hex).ok()?;
    u8::from_str_radix(hex, 16).ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 壊れたエスケープは位置によらず一貫してリテラルの `%` として扱う。
    ///
    /// 以前は「桁が足りない」場合は寛容、「桁はあるが16進として不正」な場合は
    /// `None` という位置依存の不整合があった。`None` を返すとファイル名を丸ごと
    /// 失って拡張子なしの `attachment` になるため、寛容側に統一している。
    #[test]
    fn percent_decode_treats_broken_escapes_as_literal() {
        // 末尾で切れている
        assert_eq!(percent_decode_utf8("%41%"), Some("A%".to_string()));
        // 桁が足りない
        assert_eq!(percent_decode_utf8("%4"), Some("%4".to_string()));
        assert_eq!(percent_decode_utf8("%"), Some("%".to_string()));
        // 桁はあるが16進として不正(以前は None を返していた)
        assert_eq!(percent_decode_utf8("%4X"), Some("%4X".to_string()));
        assert_eq!(percent_decode_utf8("a%ZZb"), Some("a%ZZb".to_string()));
        // 符号付き数値として解釈されないこと(from_str_radix は "+4" を受け付ける)
        assert_eq!(percent_decode_utf8("%+4"), Some("%+4".to_string()));
        assert_eq!(percent_decode_utf8("% 4"), Some("% 4".to_string()));
        // エンコードし忘れたパーセントを含む実際にありうるファイル名
        assert_eq!(
            percent_decode_utf8("100%.txt"),
            Some("100%.txt".to_string())
        );
    }

    #[test]
    fn percent_decode_handles_valid_escapes() {
        assert_eq!(percent_decode_utf8("%41%42"), Some("AB".to_string()));
        // 大文字・小文字どちらの16進数も受ける
        assert_eq!(percent_decode_utf8("%e5%9b%b3"), Some("図".to_string()));
        assert_eq!(percent_decode_utf8("%E5%9B%B3"), Some("図".to_string()));
        // 正しくエンコードされたパーセント
        assert_eq!(
            percent_decode_utf8("100%25.txt"),
            Some("100%.txt".to_string())
        );
        assert_eq!(percent_decode_utf8(""), Some(String::new()));
    }

    /// デコード結果が UTF-8 として不正な場合だけ `None` を返す。
    #[test]
    fn percent_decode_rejects_invalid_utf8() {
        assert_eq!(percent_decode_utf8("%FF"), None);
        assert_eq!(percent_decode_utf8("%E5%9B"), None);
    }

    #[test]
    fn parses_plain_filename() {
        assert_eq!(
            parse_content_disposition_filename(r#"attachment; filename="spec.pdf""#),
            Some("spec.pdf".to_string())
        );
    }

    #[test]
    fn parses_filename_without_quotes() {
        assert_eq!(
            parse_content_disposition_filename("attachment; filename=spec.pdf"),
            Some("spec.pdf".to_string())
        );
    }

    #[test]
    fn prefers_rfc5987_utf8_filename() {
        // Backlogは日本語ファイル名でこの形式を使う
        let value = "attachment; filename=\"_.png\"; filename*=UTF-8''%E5%9B%B3%E9%9D%A2.png";
        assert_eq!(
            parse_content_disposition_filename(value),
            Some("図面.png".to_string())
        );
    }

    #[test]
    fn returns_none_without_filename() {
        assert_eq!(parse_content_disposition_filename("inline"), None);
        assert_eq!(parse_content_disposition_filename(""), None);
    }

    #[test]
    fn too_large_error_message_includes_size_and_limit() {
        let err = crate::Error::TooLarge {
            size: 1000,
            limit: 500,
        };
        let text = err.to_string();
        assert!(text.contains("1000"), "got: {text}");
        assert!(text.contains("500"), "got: {text}");
    }
}
