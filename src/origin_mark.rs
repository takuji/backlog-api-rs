//! ダウンロードしたファイルに「インターネット由来」の印を付ける。
//!
//! Windows は NTFS 代替データストリーム `{path}:Zone.Identifier`、
//! macOS は拡張属性 `com.apple.quarantine` を使う。これにより
//! SmartScreen / Gatekeeper がファイルを外部由来として扱う。
//!
//! プラットフォーム固有なのは実際の書き込み1回だけで、書き込む値の組み立ては
//! すべて純粋関数にしてある(全環境のテストで検証できるようにするため)。

use std::path::Path;
#[cfg(any(windows, test))]
use std::path::PathBuf;

/// インターネット由来の印を付けた結果。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OriginMark {
    /// 印を付けた。
    Applied,
    /// このプラットフォームには相当する仕組みがない。
    Unsupported,
    /// 付けようとして失敗した。ファイル自体は保存されている。
    Failed(String),
}

/// `path` にインターネット由来の印を付ける。
///
/// 失敗しても `Err` は返さない。ダウンロード自体は成功しているため、
/// 呼び出し側は結果を報告するだけでファイルは有効として扱う。
/// (exFAT / FAT32 は代替データストリームも拡張属性も持てない)
///
/// **`path` は既存のファイルを指していなければならない。** 存在しないパスを
/// 渡したときの挙動がプラットフォームで異なるため: macOS では `xattr::set` が
/// 単に `Failed` を返すだけだが、Windows は `{path}:Zone.Identifier` への
/// `std::fs::write` が `CREATE_ALWAYS` で開くため、存在しないベースファイルと
/// ストリームの両方を新規作成してしまい `Applied` を返す(0バイトの幽霊ファイルが
/// できる)。現状の唯一の呼び出し元 `save_to` では一時ファイルを書き込み・flush・
/// close した直後のパスを渡しており、名前も pid とカウンタで一意にしているため
/// この経路には到達しない。存在チェックはここでは行わない(全ダウンロードに
/// stat を1回増やすコストに見合わないため)。`apply` を `pub` にする、あるいは
/// 保存済みファイルへ後付けで印を付ける用途に転用する場合は、この前提が崩れて
/// 実際にバグになりうる点に注意すること。
pub(crate) fn apply(path: &Path) -> OriginMark {
    #[cfg(windows)]
    {
        match std::fs::write(ads_path_for(path), zone_identifier_contents()) {
            Ok(()) => OriginMark::Applied,
            Err(e) => OriginMark::Failed(e.to_string()),
        }
    }
    #[cfg(target_os = "macos")]
    {
        // 時計が UNIXエポックより前を指している異常時は 0 を使う。
        // 印を付けないより、時刻が不正確でも付けるほうがよい。
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);
        match xattr::set(
            path,
            "com.apple.quarantine",
            quarantine_value(now).as_bytes(),
        ) {
            Ok(()) => OriginMark::Applied,
            Err(e) => OriginMark::Failed(e.to_string()),
        }
    }
    #[cfg(not(any(windows, target_os = "macos")))]
    {
        let _ = path;
        OriginMark::Unsupported
    }
}

/// Zone.Identifier ストリームの中身。
///
/// **URLは含めない。** `HostUrl=` / `ReferrerUrl=` を書くと、スペース名や
/// 課題キーといった内部情報がディスク上に平文で残る。呼び出し側がAPIキー付きの
/// URLを組んで渡す余地もあり、その場合は認証情報がそのまま残ってしまう。
/// `ZoneId=3`(インターネットゾーン)だけで SmartScreen は動作する。
#[cfg(any(windows, test))]
fn zone_identifier_contents() -> &'static str {
    "[ZoneTransfer]\r\nZoneId=3\r\n"
}

/// `{path}:Zone.Identifier` を組み立てる。
#[cfg(any(windows, test))]
fn ads_path_for(path: &Path) -> PathBuf {
    let mut ads = path.to_path_buf().into_os_string();
    ads.push(":Zone.Identifier");
    PathBuf::from(ads)
}

/// `com.apple.quarantine` の値を組み立てる。`now_secs` は UNIXエポック秒。
#[cfg(any(target_os = "macos", test))]
fn quarantine_value(now_secs: u64) -> String {
    // フラグは 0001(QTN_FLAG_DOWNLOAD)のみ。0x40(ユーザー承認済み)や
    // 0x80(Gatekeeper検査済み)を立てると印を付ける意味が消える。
    // 第4フィールドのイベントUUIDは空にする(Gatekeeperの判定には不要で、
    // UUID生成のための依存を増やしたくない)。
    format!("0001;{now_secs:08x};backlog-api-rs;")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn zone_identifier_declares_the_internet_zone_with_crlf() {
        let contents = zone_identifier_contents();
        assert!(contents.contains("[ZoneTransfer]"), "got: {contents:?}");
        assert!(contents.contains("ZoneId=3"), "got: {contents:?}");
        // Windows の ini 形式は CRLF が期待される
        assert!(contents.contains("\r\n"), "got: {contents:?}");
    }

    /// Zone.Identifier に URL を書かないという決定を固定する。
    ///
    /// `HostUrl=` や `ReferrerUrl=` を足すと、スペース名や課題キー、そして
    /// 呼び出し側がURLに認証情報を載せていた場合はそれごと、ディスク上の
    /// ファイルに平文で残る。このプロジェクトは同種のAPIキー漏洩をすでに
    /// 三度修正している。
    #[test]
    fn zone_identifier_never_contains_a_url() {
        let contents = zone_identifier_contents();
        // 大文字・小文字の揺れ(`HostUrl=` や `HTTP://` など)で見逃さないよう、
        // 小文字化してから比較する。
        let lower = contents.to_ascii_lowercase();
        assert!(!lower.contains("http"), "got: {contents:?}");
        assert!(!lower.contains("url"), "got: {contents:?}");
    }

    #[test]
    fn ads_path_appends_the_stream_name() {
        assert_eq!(
            ads_path_for(Path::new("/tmp/out.txt.1234-0.part")),
            PathBuf::from("/tmp/out.txt.1234-0.part:Zone.Identifier")
        );
        // 拡張子の置換ではなく末尾への追加であること
        assert_eq!(
            ads_path_for(Path::new("/tmp/図面.png")),
            PathBuf::from("/tmp/図面.png:Zone.Identifier")
        );
    }

    /// 実データで確認した形式に一致すること。
    ///
    /// このマシンの Chrome 由来のアプリが持つ値は
    /// `01c1;6a0fd109;Chrome;451BDBF6-...` という形をしていた。
    #[test]
    fn quarantine_value_matches_the_observed_format() {
        assert_eq!(
            quarantine_value(0x6a0f_d109),
            "0001;6a0fd109;backlog-api-rs;"
        );
        // 4フィールド(末尾のイベントUUIDは空)
        assert_eq!(quarantine_value(1).split(';').count(), 4);
        // 時刻は小文字16進で、10進表記になっていないこと
        assert!(
            quarantine_value(0x00ab_cdef).contains("00abcdef"),
            "got: {}",
            quarantine_value(0x00ab_cdef)
        );
    }

    /// フラグが厳密に `0001` であることを固定する。
    ///
    /// `0x40`(QTN_FLAG_USER_APPROVED)や `0x80`(QTN_FLAG_ASSESSMENT_OK)を
    /// 立てると「ユーザーが承認済み」「Gatekeeper が検査済み」と偽ることになり、
    /// 印を付ける目的そのものが消える。ネット上には `0081` / `0083` を使う
    /// 例があるため、値が変わったら必ずテストが落ちるようにしておく。
    #[test]
    fn quarantine_flags_claim_only_download_origin() {
        let value = quarantine_value(0x6a0f_d109);
        let flags = value.split(';').next().unwrap();
        assert_eq!(flags, "0001", "got: {value}");

        let bits = u32::from_str_radix(flags, 16).unwrap();
        assert_eq!(bits & 0x0001, 0x0001, "ダウンロード由来のビットが必要");
        assert_eq!(bits & 0x0040, 0, "ユーザー承認済みと偽ってはならない");
        assert_eq!(bits & 0x0080, 0, "Gatekeeper検査済みと偽ってはならない");
    }

    /// macOS では実際に拡張属性が書けることを確認する。
    ///
    /// `apply` のプラットフォーム固有部分のうち、macOS 経路だけは
    /// 開発機で実行できる(Windows の ADS 経路は検証できない)。
    #[cfg(target_os = "macos")]
    #[test]
    fn apply_sets_the_quarantine_attribute_on_macos() {
        let dir = std::env::temp_dir().join(format!(
            "backlog-api-rs-origin-mark-{}-{}",
            std::process::id(),
            line!()
        ));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("marked.txt");
        std::fs::write(&path, b"payload").unwrap();

        assert_eq!(apply(&path), OriginMark::Applied);

        let value = xattr::get(&path, "com.apple.quarantine")
            .unwrap()
            .expect("quarantine 属性が設定されていない");
        let value = String::from_utf8(value).unwrap();
        assert!(value.starts_with("0001;"), "got: {value}");
        assert!(value.ends_with(";backlog-api-rs;"), "got: {value}");

        std::fs::remove_dir_all(&dir).ok();
    }

    /// 書き込めない相手では `Failed` を返し、panic しないこと。
    ///
    /// 拡張属性を持てないファイルシステム(exFAT等)をテストで用意できないため、
    /// 存在しないパスでエラー変換の経路を代替検証する。
    #[cfg(target_os = "macos")]
    #[test]
    fn apply_reports_failure_without_panicking() {
        let missing = std::env::temp_dir()
            .join("backlog-api-rs-no-such-dir")
            .join("no-such-file.txt");
        assert!(matches!(apply(&missing), OriginMark::Failed(_)));
    }
}
