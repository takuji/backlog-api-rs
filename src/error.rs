use serde::Deserialize;

/// backlog-api-rs のエラー型。
#[derive(Debug, thiserror::Error)]
pub enum Error {
    /// 通信エラー・タイムアウト・JSONデコードエラーなど reqwest 由来のエラー。
    ///
    /// `reqwest::Error` の Display/Debug にはリクエストURLが含まれる。
    /// APIキーはヘッダ(`Backlog-API-Key`)で送るのでURLには載らないが、
    /// 呼び出し側が `apiKey` 付きのURLを組んで渡す余地は残るため、
    /// `From` 実装で `without_url()` によりURLを除去してから包む。
    #[error("http error: {}", describe_source_chain(_0))]
    Http(reqwest::Error),

    /// Backlog API がエラーレスポンス(`{"errors": [...]}`)を返した。
    #[error("backlog api error (status {status}): {}", format_messages(errors))]
    Api { status: u16, errors: Vec<ApiError> },

    /// 2xx以外だがエラーレスポンスとしてパースできなかった。
    #[error("unexpected response (status {status}): {body}")]
    UnexpectedResponse { status: u16, body: String },

    /// ベースURLが不正。
    #[error("invalid base url: {0}")]
    Url(#[from] url::ParseError),

    /// ダウンロードサイズが上限を超えた。
    #[error("file too large: {size} bytes exceeds the limit of {limit} bytes")]
    TooLarge { size: u64, limit: u64 },

    /// ファイルの読み書きに失敗した。
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),

    /// リダイレクトが上限回数を超えた。
    ///
    /// リダイレクトは reqwest に任せず自分で追っている(APIキーを外部ホストへ
    /// 運ばないため)。ループを検出する役目もこちらに移っている。
    #[error("too many redirects (limit {limit})")]
    TooManyRedirects { limit: usize },
}

impl From<reqwest::Error> for Error {
    fn from(err: reqwest::Error) -> Self {
        // URLに認証情報が載っていた場合にDisplay/Debugへ漏れないよう取り除く。
        // ホスト・パス情報も失われるが安全性を優先する。
        Error::Http(err.without_url())
    }
}

/// エラーとその原因チェーンを連結する。
///
/// reqwest の Display は "error decoding response body" のように概要しか出さないため、
/// serde の「どのフィールドが期待と違うか」という情報が失われる。それを表に出す。
fn describe_source_chain(err: &reqwest::Error) -> String {
    let mut message = err.to_string();
    let mut source = std::error::Error::source(err);
    while let Some(e) = source {
        message.push_str(": ");
        message.push_str(&e.to_string());
        source = e.source();
    }
    message
}

fn format_messages(errors: &[ApiError]) -> String {
    errors
        .iter()
        .map(|e| e.message.as_str())
        .collect::<Vec<_>>()
        .join("; ")
}

/// Backlogのエラーレスポンス中の個々のエラー。
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ApiError {
    pub message: String,
    pub code: ErrorCode,
    #[serde(default)]
    pub more_info: Option<String>,
}

/// Backlog APIのエラーコード。
/// <https://developer.nulab.com/docs/backlog/error-response/>
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(from = "i64")]
pub enum ErrorCode {
    Internal,
    Licence,
    LicenceExpired,
    AccessDenied,
    UnauthorizedOperation,
    NoResource,
    InvalidRequest,
    SpaceOverCapacity,
    ResourceOverflow,
    TooLargeFile,
    Authentication,
    RequiredMfa,
    TooManyRequests,
    /// 仕様に載っていないコード。
    Unknown(i64),
}

impl From<i64> for ErrorCode {
    fn from(code: i64) -> Self {
        match code {
            1 => ErrorCode::Internal,
            2 => ErrorCode::Licence,
            3 => ErrorCode::LicenceExpired,
            4 => ErrorCode::AccessDenied,
            5 => ErrorCode::UnauthorizedOperation,
            6 => ErrorCode::NoResource,
            7 => ErrorCode::InvalidRequest,
            8 => ErrorCode::SpaceOverCapacity,
            9 => ErrorCode::ResourceOverflow,
            10 => ErrorCode::TooLargeFile,
            11 => ErrorCode::Authentication,
            12 => ErrorCode::RequiredMfa,
            13 => ErrorCode::TooManyRequests,
            other => ErrorCode::Unknown(other),
        }
    }
}

/// Backlogのエラーレスポンス全体(`{"errors": [...]}`)。
#[derive(Debug, Deserialize)]
pub(crate) struct ErrorResponse {
    pub errors: Vec<ApiError>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn deserializes_error_response() {
        let json = r#"{
            "errors": [
                { "message": "No project.", "code": 6, "moreInfo": "" }
            ]
        }"#;
        let resp: ErrorResponse = serde_json::from_str(json).unwrap();
        assert_eq!(resp.errors.len(), 1);
        assert_eq!(resp.errors[0].message, "No project.");
        assert_eq!(resp.errors[0].code, ErrorCode::NoResource);
    }

    #[test]
    fn unknown_error_code_is_preserved() {
        let json = r#"{ "errors": [{ "message": "x", "code": 99 }] }"#;
        let resp: ErrorResponse = serde_json::from_str(json).unwrap();
        assert_eq!(resp.errors[0].code, ErrorCode::Unknown(99));
        assert_eq!(resp.errors[0].more_info, None);
    }

    #[test]
    fn api_error_display_includes_messages() {
        let err = Error::Api {
            status: 404,
            errors: vec![ApiError {
                message: "No such issue.".to_string(),
                code: ErrorCode::NoResource,
                more_info: None,
            }],
        };
        let s = err.to_string();
        assert!(s.contains("404"), "got: {s}");
        assert!(s.contains("No such issue."), "got: {s}");
    }
}
