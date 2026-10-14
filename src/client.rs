use reqwest::Method;
use serde::de::DeserializeOwned;
use url::Url;

use crate::error::ErrorResponse;
use crate::params::QueryPairs;
use crate::{Error, Result};

/// Backlog API v2 クライアント。
///
/// ```no_run
/// use backlog_api_rs::BacklogClient;
///
/// # async fn example() -> backlog_api_rs::Result<()> {
/// let client = BacklogClient::new("https://example.backlog.com", "API_KEY")?;
/// # Ok(())
/// # }
/// ```
#[derive(Debug, Clone)]
pub struct BacklogClient {
    http: reqwest::Client,
    base_url: Url,
    auth: Auth,
}

/// 認証方式。現状APIキーのみ。OAuth2対応時にバリアントを追加する。
#[derive(Clone)]
enum Auth {
    ApiKey(String),
}

impl std::fmt::Debug for Auth {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Auth::ApiKey(_) => f.write_str("ApiKey(REDACTED)"),
        }
    }
}

/// `/api/v2` 配下のパスを、ユーザー入力(`issue_id_or_key` など)を
/// percent-encodeしつつ組み立てる。
///
/// `segments` の各要素は1つのパスセグメントとして扱われ、`/`・`?`・`#` などの
/// 区切り文字はエンコードされるため、`"BLG-1?x=y"` のような入力がクエリ注入や
/// `"../projects"` のような入力が別エンドポイントへのパストラバーサルに
/// つながることはない。
///
/// 注意: `url` クレートは WHATWG URL 仕様に従い、パスセグメントが(percent-encode
/// 後の値も含めて)`.` または `..` と等価と判定される場合、`push()` の時点で
/// 直前のセグメントを消費してしまう(ドットセグメント正規化)。これは
/// `Url::join` でも同様に発生するため、単純なpercent-encodeでは防げない。
/// そのため、セグメントがドットのみで構成される場合は `%` 自体を二重に
/// エンコードし、正規化の対象にならない値に変換してから追加する。
pub(crate) fn api_path(segments: &[&str]) -> String {
    let mut url = Url::parse("https://placeholder.invalid/api/v2").expect("static url is valid");
    {
        let mut path_segments = url
            .path_segments_mut()
            .expect("static base is not cannot-be-a-base");
        for segment in segments {
            if is_dot_only(segment) {
                // "." や ".." をそのまま push() すると(url crateの仕様により)
                // ドットセグメントとして正規化され、直前のセグメントが
                // 消えてしまう。"%2E" 表記にしてから push() することで、
                // push() 自身の '%' エンコードにより "%252E" のような
                // 二重エンコード値になり、正規化の対象外になる。
                let escaped: String = segment.chars().map(|_| "%2E").collect();
                path_segments.push(&escaped);
            } else {
                path_segments.push(segment);
            }
        }
    }
    url.path().to_string()
}

/// `s` が空でなく、`.` のみで構成されているか(`"."`, `".."`, `"..."` など)。
fn is_dot_only(s: &str) -> bool {
    !s.is_empty() && s.bytes().all(|b| b == b'.')
}

/// レスポンスのチャンクが届かないまま経過してよい時間。
/// 全体の制限時間ではないため、大きなファイルの正常な転送は妨げない。
const READ_IDLE_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(60);

/// このクライアントが名乗る `User-Agent`。
///
/// reqwest は既定で `User-Agent` を**まったく送らない**。UA を持たないリクエストは
/// WAF やプロキシに機械的に落とされることがあるうえ、サーバー側のログから
/// 問い合わせの出所を辿れなくなる。ライブラリとして既定で名乗る。
const USER_AGENT: &str = concat!("backlog-api-rs/", env!("CARGO_PKG_VERSION"));

/// APIキーを載せるHTTPヘッダ。
///
/// Backlog API v2 はクエリパラメータ `apiKey` でも受け付けるが、クエリ文字列は
/// アクセスログ・`Referer`・ブラウザ履歴・エラーメッセージのURLと、値が意図せず
/// 転記される経路が多い。ヘッダで送ればそれらの経路には載らない。
const API_KEY_HEADER: &str = "Backlog-API-Key";

/// 手動でリダイレクトを追う上限。reqwest の既定と同じ。
const MAX_REDIRECTS: usize = 10;

/// [`BacklogClient`] の構築を組み立てる。
///
/// [`BacklogClient::builder`] から作る。設定が要らなければ
/// [`BacklogClient::new`] を使えばよい。
pub struct BacklogClientBuilder {
    base_url: String,
    api_key: String,
    user_agent: Option<String>,
}

impl BacklogClientBuilder {
    /// `User-Agent` に呼び出し側の名前を載せる。
    ///
    /// **ライブラリ自身のトークンは後ろに残る。** 例えば `"backlog-inbox/0.4.7"` を
    /// 渡すと、実際に送られるのは
    /// `backlog-inbox/0.4.7 backlog-api-rs/<version>` になる。
    /// HTTP の `User-Agent` はもともと製品トークンの列(RFC 9110 §10.1.5)であり、
    /// これでサーバーのログから「どのアプリか」と「クライアントのどの版か」の
    /// 両方が分かる。
    ///
    /// 前後の空白は落とす。落とした結果が空なら指定しなかったのと同じ扱いになる
    /// (先頭に空白の付いた `User-Agent` を送らないため)。
    ///
    /// 改行のようにヘッダ値として使えない文字が含まれていると
    /// [`Self::build`] が `Err` を返す。値の検証は reqwest 側で行われる。
    pub fn user_agent(mut self, product: impl Into<String>) -> Self {
        self.user_agent = Some(product.into());
        self
    }

    /// クライアントを組み立てる。
    pub fn build(self) -> Result<BacklogClient> {
        let http = reqwest::Client::builder()
            .read_timeout(READ_IDLE_TIMEOUT)
            .user_agent(self.resolved_user_agent())
            // リダイレクトは reqwest に追わせず、[`BacklogClient::send`] で自分で追う。
            //
            // reqwest の自動追従は独自ヘッダをそのまま次のホップへ運ぶ
            // (剥がされるのは `Authorization` や `Cookie` など既知のものだけ)。
            // APIキーは独自ヘッダ `Backlog-API-Key` で送るため、任せると
            // **キーごと外部ホストに渡ってしまう**。redirect policy からは
            // ヘッダを触れないので、追従そのものを手元に持ってくる。
            //
            // これは机上の懸念ではない。`GET /users/{id}/icon` は、アイコンが
            // Nulabアカウント側にある場合にサーバーが外部URLへ 302 を返す
            // (サーバー実装 `space/Users.scala` の `NulabAccountIconResource`)。
            .redirect(reqwest::redirect::Policy::none())
            // `Referer` は reqwest がリダイレクト追従時に付けるものなので、
            // `Policy::none()` の下では出番がない。追従の実装を変えたときに
            // 黙って復活しないよう、切ったままにしておく。
            .referer(false)
            .build()?;
        Ok(BacklogClient {
            http,
            base_url: Url::parse(&self.base_url)?,
            auth: Auth::ApiKey(self.api_key),
        })
    }

    /// 実際に送る `User-Agent` を組み立てる。
    fn resolved_user_agent(&self) -> String {
        match self.user_agent.as_deref().map(str::trim) {
            Some(product) if !product.is_empty() => format!("{product} {USER_AGENT}"),
            _ => USER_AGENT.to_string(),
        }
    }
}

impl BacklogClient {
    /// クライアントを作る。`base_url` は `https://<space>.backlog.com` 形式。
    ///
    /// `User-Agent` などを指定したい場合は [`Self::builder`] を使う。
    pub fn new(base_url: &str, api_key: impl Into<String>) -> Result<Self> {
        Self::builder(base_url, api_key).build()
    }

    /// 設定を指定してクライアントを作る。
    ///
    /// **`new` はこれの上に載っている。** reqwest の設定を2か所に持つと、
    /// 片方で `referer(false)` を落としたときに気づけない。あれはAPIキーが
    /// リダイレクト先へ漏れるのを塞いでいる設定なので、構築経路は1本にしてある。
    ///
    /// ```no_run
    /// use backlog_api_rs::BacklogClient;
    ///
    /// # fn example() -> backlog_api_rs::Result<()> {
    /// let client = BacklogClient::builder("https://example.backlog.com", "API_KEY")
    ///     .user_agent("my-app/1.0")
    ///     .build()?;
    /// # Ok(())
    /// # }
    /// ```
    pub fn builder(base_url: &str, api_key: impl Into<String>) -> BacklogClientBuilder {
        BacklogClientBuilder {
            base_url: base_url.to_string(),
            api_key: api_key.into(),
            user_agent: None,
        }
    }

    /// ユーザー関連API。
    pub fn users(&self) -> crate::api::UsersApi<'_> {
        crate::api::UsersApi::new(self)
    }

    /// スペース全体の参照データAPI(優先度・完了理由)。
    pub fn space(&self) -> crate::api::SpaceApi<'_> {
        crate::api::SpaceApi::new(self)
    }

    /// プロジェクト関連API。
    pub fn projects(&self) -> crate::api::ProjectsApi<'_> {
        crate::api::ProjectsApi::new(self)
    }

    /// 課題関連API。
    pub fn issues(&self) -> crate::api::IssuesApi<'_> {
        crate::api::IssuesApi::new(self)
    }

    /// Wiki関連API。
    pub fn wikis(&self) -> crate::api::WikisApi<'_> {
        crate::api::WikisApi::new(self)
    }

    /// ドキュメント関連API。
    pub fn documents(&self) -> crate::api::DocumentsApi<'_> {
        crate::api::DocumentsApi::new(self)
    }

    /// お知らせ関連API。
    pub fn notifications(&self) -> crate::api::NotificationsApi<'_> {
        crate::api::NotificationsApi::new(self)
    }

    /// ウォッチ関連API。
    pub fn watchings(&self) -> crate::api::WatchingsApi<'_> {
        crate::api::WatchingsApi::new(self)
    }

    /// Git / プルリクエスト関連API。
    pub fn git(&self) -> crate::api::GitApi<'_> {
        crate::api::GitApi::new(self)
    }

    pub(crate) async fn get<T: DeserializeOwned>(
        &self,
        path: &str,
        query: QueryPairs,
    ) -> Result<T> {
        self.execute(Method::GET, path, query, QueryPairs::new())
            .await
    }

    /// 一覧を1件ずつデシリアライズして取得する。
    ///
    /// 壊れた要素があっても呼び出し全体を失敗させず、良い要素を返して
    /// 落とした事実を [`PartialList::skipped`] に記録する。
    ///
    /// **本文が配列でなければ従来どおり失敗する。** 認証エラーの本文や
    /// プロキシのHTMLを「0件の一覧」として扱うと問題が見えなくなるため。
    pub(crate) async fn get_list<T: DeserializeOwned>(
        &self,
        path: &str,
        query: QueryPairs,
    ) -> Result<crate::PartialList<T>> {
        let values: Vec<serde_json::Value> = self
            .execute(Method::GET, path, query, QueryPairs::new())
            .await?;
        Ok(crate::PartialList::from_values(values))
    }

    pub(crate) async fn post<T: DeserializeOwned>(
        &self,
        path: &str,
        form: QueryPairs,
    ) -> Result<T> {
        self.execute(Method::POST, path, QueryPairs::new(), form)
            .await
    }

    /// 応答本文を持たない POST(204 No Content)。
    ///
    /// 通知の個別既読化(`POST /notifications/{id}/markAsRead`)が該当する。
    /// サーバー実装は `NoContent` を返すため、[`Self::post`] のように
    /// JSON としてデシリアライズしようとすると本文が空で失敗する。
    pub(crate) async fn post_no_content(&self, path: &str) -> Result<()> {
        self.execute_raw(Method::POST, path, QueryPairs::new(), QueryPairs::new())
            .await?;
        Ok(())
    }

    pub(crate) async fn patch<T: DeserializeOwned>(
        &self,
        path: &str,
        form: QueryPairs,
    ) -> Result<T> {
        self.execute(Method::PATCH, path, QueryPairs::new(), form)
            .await
    }

    pub(crate) async fn delete<T: DeserializeOwned>(&self, path: &str) -> Result<T> {
        self.execute(Method::DELETE, path, QueryPairs::new(), QueryPairs::new())
            .await
    }

    /// バイナリのダウンロードを開始する(ヘッダーだけ受信し、ボディは未読)。
    ///
    /// 通常の経路(`execute`)はJSONを前提にしているため、バイナリ用に別経路を用意する。
    /// 認証・エラー変換は同じ扱い。ボディは [`crate::AttachmentDownload::save_to`] で
    /// ストリーミング書き込みする。
    pub(crate) async fn start_download(&self, path: &str) -> Result<crate::AttachmentDownload> {
        let url = self.base_url.join(path)?;
        let response = self.send(Method::GET, url, &QueryPairs::new()).await?;
        let status = response.status();

        if !status.is_success() {
            let body = response.text().await.unwrap_or_default();
            return match serde_json::from_str::<ErrorResponse>(&body) {
                Ok(parsed) => Err(Error::Api {
                    status: status.as_u16(),
                    errors: parsed.errors,
                }),
                Err(_) => Err(Error::UnexpectedResponse {
                    status: status.as_u16(),
                    body,
                }),
            };
        }

        Ok(crate::AttachmentDownload::new(response))
    }

    /// APIキーを載せてリクエストを送り、GET のリダイレクトは自分で追う。
    ///
    /// **APIキーを載せるのは、宛先がベースURLと同一オリジンのときだけ。**
    /// リダイレクト先が外部ホスト(Nulabアカウントのアイコン配信など)の場合、
    /// そこにAPIキーを渡す理由はなく、渡せば漏洩になる。reqwest の自動追従は
    /// 独自ヘッダを剥がしてくれないため、この判断は手元でしかできない。
    ///
    /// 追うのは GET だけ。Backlog の書き込み系APIはリダイレクトを返さないので、
    /// 返ってきたら異常としてそのまま呼び出し側のエラー処理に流す
    /// (本文ごと見えた方が原因を追える)。`Location` が無い・ヘッダ値として
    /// 読めない 3xx も同じ扱いにする。
    async fn send(&self, method: Method, url: Url, form: &QueryPairs) -> Result<reqwest::Response> {
        let mut url = url;
        for _ in 0..=MAX_REDIRECTS {
            let mut request = self.http.request(method.clone(), url.clone());
            if url.origin() == self.base_url.origin() {
                let Auth::ApiKey(key) = &self.auth;
                request = request.header(API_KEY_HEADER, key);
            }
            if !form.is_empty() {
                request = request.form(form.as_slice());
            }
            let response = request.send().await?;

            if method != Method::GET || !response.status().is_redirection() {
                return Ok(response);
            }
            let Some(location) = response
                .headers()
                .get(reqwest::header::LOCATION)
                .and_then(|value| value.to_str().ok())
            else {
                return Ok(response);
            };
            url = url.join(location)?;
        }
        Err(Error::TooManyRedirects {
            limit: MAX_REDIRECTS,
        })
    }

    async fn execute<T: DeserializeOwned>(
        &self,
        method: Method,
        path: &str,
        query: QueryPairs,
        form: QueryPairs,
    ) -> Result<T> {
        let response = self.execute_raw(method, path, query, form).await?;
        Ok(response.json().await?)
    }

    /// 応答本文を読まずに `reqwest::Response` を返す。
    ///
    /// 成功以外のステータスの扱いは [`Self::execute`] と共通にしたいが、
    /// 本文の解釈は呼び出し側で変えたい場合に使う。具体的には
    /// 204 No Content を返すエンドポイント(通知の個別既読化)と、
    /// バイト列を返すエンドポイント(添付・アイコン)。
    async fn execute_raw(
        &self,
        method: Method,
        path: &str,
        query: QueryPairs,
        form: QueryPairs,
    ) -> Result<reqwest::Response> {
        let mut url = self.base_url.join(path)?;
        // 空のまま `query_pairs_mut()` を呼ぶと、パラメータを1つも積まなくても
        // 末尾に `?` の付いたURLになる。積むものがあるときだけ触る。
        if !query.is_empty() {
            let mut qp = url.query_pairs_mut();
            for (k, v) in query.as_slice() {
                qp.append_pair(k, v);
            }
        }

        let response = self.send(method, url, &form).await?;

        let status = response.status();
        if status.is_success() {
            return Ok(response);
        }

        let body = response.text().await.unwrap_or_default();
        match serde_json::from_str::<ErrorResponse>(&body) {
            Ok(parsed) => Err(Error::Api {
                status: status.as_u16(),
                errors: parsed.errors,
            }),
            Err(_) => Err(Error::UnexpectedResponse {
                status: status.as_u16(),
                body,
            }),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use wiremock::matchers::{body_string_contains, header, method, path, query_param};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    #[derive(Debug, serde::Deserialize)]
    struct Pong {
        ok: bool,
    }

    /// リクエストに `User-Agent` が付くこと。
    ///
    /// reqwest は既定で UA を送らない。UA 無しのリクエストは WAF や
    /// プロキシに落とされることがあるため、既定で名乗ることを固定する。
    #[tokio::test]
    async fn sends_a_user_agent() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/api/v2/ping"))
            .and(header(
                "user-agent",
                concat!("backlog-api-rs/", env!("CARGO_PKG_VERSION")),
            ))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({ "ok": true })))
            .expect(1)
            .mount(&server)
            .await;

        let client = BacklogClient::new(&server.uri(), "KEY").unwrap();
        let pong: Pong = client.get("/api/v2/ping", QueryPairs::new()).await.unwrap();
        assert!(pong.ok);
    }

    /// `User-Agent` の組み立てを直接固定する。
    ///
    /// **ワイヤ越しのテストではこれを検証できない。** HTTP のフィールド値は
    /// 前後の空白(OWS)を受信側が剥がすので(RFC 9110 §5.5)、
    /// `" backlog-api-rs/x"` を送ってもサーバーには `"backlog-api-rs/x"` として
    /// 届き、モックに一致してしまう。空指定の扱いを間違えても気づけないため、
    /// ここで組み立て結果そのものを見る。
    #[test]
    fn resolves_the_user_agent() {
        let library = format!("backlog-api-rs/{}", env!("CARGO_PKG_VERSION"));
        let builder = || BacklogClient::builder("https://example.backlog.com", "KEY");

        // 未指定
        assert_eq!(builder().resolved_user_agent(), library);
        // 呼び出し側の名前が前、ライブラリが後ろ
        assert_eq!(
            builder().user_agent("my-app/1.0").resolved_user_agent(),
            format!("my-app/1.0 {library}")
        );
        // 前後の空白は落ちる
        assert_eq!(
            builder().user_agent("  my-app/1.0  ").resolved_user_agent(),
            format!("my-app/1.0 {library}")
        );
        // 空・空白のみは未指定と同じ(先頭に空白の付いた値を送らない)
        for blank in ["", " ", "   ", "\t", "\n"] {
            assert_eq!(
                builder().user_agent(blank).resolved_user_agent(),
                library,
                "blank = {blank:?}"
            );
        }
    }

    /// 呼び出し側の名前が**前**に来て、ライブラリのトークンが後ろに残ること。
    ///
    /// 期待値は `env!` とリテラルから組む。`USER_AGENT` 定数や
    /// `resolved_user_agent` から作ると、被検証コードで期待値を作ることになり
    /// 組み立ての誤りを検出できない。
    #[tokio::test]
    async fn builder_puts_the_caller_product_before_the_library_token() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/api/v2/ping"))
            .and(wiremock::matchers::header(
                "user-agent",
                format!("my-app/1.0 backlog-api-rs/{}", env!("CARGO_PKG_VERSION")).as_str(),
            ))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({ "ok": true })))
            .expect(1)
            .mount(&server)
            .await;

        let client = BacklogClient::builder(&server.uri(), "KEY")
            .user_agent("my-app/1.0")
            .build()
            .unwrap();
        let pong: Pong = client.get("/api/v2/ping", QueryPairs::new()).await.unwrap();
        assert!(pong.ok);
    }

    /// 空文字列・空白のみは未指定と同じ扱いになること。
    ///
    /// そのまま連結すると先頭に空白の付いた `User-Agent` を送ってしまう。
    #[tokio::test]
    async fn a_blank_user_agent_is_treated_as_unset() {
        for blank in ["", "   ", "\t"] {
            let server = MockServer::start().await;
            Mock::given(method("GET"))
                .and(path("/api/v2/ping"))
                .and(wiremock::matchers::header(
                    "user-agent",
                    format!("backlog-api-rs/{}", env!("CARGO_PKG_VERSION")).as_str(),
                ))
                .respond_with(ResponseTemplate::new(200).set_body_json(json!({ "ok": true })))
                .expect(1)
                .mount(&server)
                .await;

            let client = BacklogClient::builder(&server.uri(), "KEY")
                .user_agent(blank)
                .build()
                .unwrap();
            let pong: Pong = client.get("/api/v2/ping", QueryPairs::new()).await.unwrap();
            assert!(pong.ok, "blank = {blank:?}");
        }
    }

    /// 前後の空白は落ちること。
    #[tokio::test]
    async fn a_padded_user_agent_is_trimmed() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/api/v2/ping"))
            .and(wiremock::matchers::header(
                "user-agent",
                format!("my-app/1.0 backlog-api-rs/{}", env!("CARGO_PKG_VERSION")).as_str(),
            ))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({ "ok": true })))
            .expect(1)
            .mount(&server)
            .await;

        let client = BacklogClient::builder(&server.uri(), "KEY")
            .user_agent("  my-app/1.0  ")
            .build()
            .unwrap();
        let pong: Pong = client.get("/api/v2/ping", QueryPairs::new()).await.unwrap();
        assert!(pong.ok);
    }

    /// **ヘッダ値として使えない文字は通らないこと。**
    ///
    /// 改行を含む値を受け入れると、後続のヘッダを差し込まれる余地ができる。
    /// 検証は reqwest(http クレート)が行い、`build()` が `Err` になる。
    #[test]
    fn a_user_agent_that_cannot_be_a_header_value_is_rejected() {
        for bad in [
            "my-app/1.0\r\nX-Injected: 1",
            "my-app/1.0\nX-Injected: 1",
            "my-app/1.0\0",
        ] {
            let result = BacklogClient::builder("https://example.backlog.com", "KEY")
                .user_agent(bad)
                .build();
            assert!(result.is_err(), "不正なヘッダ値が通った: {bad:?}");
        }
    }

    /// `new` と `builder().build()` が同じ `User-Agent` を出すこと。
    ///
    /// 構築経路が1本であることの確認。片方だけに設定を足す退行を捕まえる
    /// (reqwest の設定が2か所に分かれると、`referer(false)` のような
    /// セキュリティ上の設定を片方で落としても気づけない)。
    #[tokio::test]
    async fn new_and_builder_send_the_same_user_agent() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/api/v2/ping"))
            .and(wiremock::matchers::header(
                "user-agent",
                format!("backlog-api-rs/{}", env!("CARGO_PKG_VERSION")).as_str(),
            ))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({ "ok": true })))
            .expect(2)
            .mount(&server)
            .await;

        let via_new = BacklogClient::new(&server.uri(), "KEY").unwrap();
        let via_builder = BacklogClient::builder(&server.uri(), "KEY")
            .build()
            .unwrap();
        let _: Pong = via_new
            .get("/api/v2/ping", QueryPairs::new())
            .await
            .unwrap();
        let _: Pong = via_builder
            .get("/api/v2/ping", QueryPairs::new())
            .await
            .unwrap();
    }

    #[tokio::test]
    async fn get_sends_the_api_key_header_and_query() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/api/v2/ping"))
            .and(header("Backlog-API-Key", "SECRET"))
            .and(query_param("count", "10"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({ "ok": true })))
            .expect(1)
            .mount(&server)
            .await;

        let client = BacklogClient::new(&server.uri(), "SECRET").unwrap();
        let mut query = QueryPairs::new();
        query.push("count", 10);
        let pong: Pong = client.get("/api/v2/ping", query).await.unwrap();
        assert!(pong.ok);
    }

    #[tokio::test]
    async fn post_sends_form_body() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/api/v2/ping"))
            .and(header("Backlog-API-Key", "SECRET"))
            .and(body_string_contains("summary=hello"))
            .respond_with(ResponseTemplate::new(201).set_body_json(json!({ "ok": true })))
            .expect(1)
            .mount(&server)
            .await;

        let client = BacklogClient::new(&server.uri(), "SECRET").unwrap();
        let mut form = QueryPairs::new();
        form.push("summary", "hello");
        let pong: Pong = client.post("/api/v2/ping", form).await.unwrap();
        assert!(pong.ok);
    }

    #[tokio::test]
    async fn error_response_becomes_api_error() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/api/v2/ping"))
            .respond_with(ResponseTemplate::new(404).set_body_json(json!({
                "errors": [{ "message": "No such issue.", "code": 6 }]
            })))
            .mount(&server)
            .await;

        let client = BacklogClient::new(&server.uri(), "SECRET").unwrap();
        let err = client
            .get::<Pong>("/api/v2/ping", QueryPairs::new())
            .await
            .unwrap_err();
        match err {
            crate::Error::Api { status, errors } => {
                assert_eq!(status, 404);
                assert_eq!(errors[0].code, crate::ErrorCode::NoResource);
            }
            other => panic!("expected Error::Api, got {other:?}"),
        }
    }

    #[tokio::test]
    async fn unparsable_error_body_is_preserved() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/api/v2/ping"))
            .respond_with(ResponseTemplate::new(502).set_body_string("Bad Gateway"))
            .mount(&server)
            .await;

        let client = BacklogClient::new(&server.uri(), "SECRET").unwrap();
        let err = client
            .get::<Pong>("/api/v2/ping", QueryPairs::new())
            .await
            .unwrap_err();
        match err {
            crate::Error::UnexpectedResponse { status, body } => {
                assert_eq!(status, 502);
                assert_eq!(body, "Bad Gateway");
            }
            other => panic!("expected UnexpectedResponse, got {other:?}"),
        }
    }

    #[tokio::test]
    async fn decode_error_includes_field_detail() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/api/v2/ping"))
            // ok は bool のはずが文字列 → serde のデコードエラーになる
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({ "ok": "yes" })))
            .mount(&server)
            .await;

        let client = BacklogClient::new(&server.uri(), "SUPER_SECRET").unwrap();
        let err = client
            .get::<Pong>("/api/v2/ping", QueryPairs::new())
            .await
            .unwrap_err();
        let text = err.to_string();
        // どのフィールドが原因か分かること(serdeはこのケースではフィールド名までは
        // 含めないため、期待型が出ることを検証する)
        assert!(text.contains("invalid type"), "got: {text}");
        assert!(text.contains("boolean"), "got: {text}");
        // APIキーは漏れないこと
        assert!(!text.contains("SUPER_SECRET"), "got: {text}");
    }

    /// 一覧の途中の1件が壊れていても、残りが返ること。
    #[tokio::test]
    async fn get_list_keeps_the_good_items_and_records_the_bad_one() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/api/v2/things"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!([
                { "ok": true }, { "ok": "not a bool" }, { "ok": false }
            ])))
            .expect(1)
            .mount(&server)
            .await;

        let client = BacklogClient::new(&server.uri(), "KEY").unwrap();
        let list: crate::PartialList<Pong> = client
            .get_list("/api/v2/things", QueryPairs::new())
            .await
            .unwrap();
        assert_eq!(list.items.len(), 2);
        assert!(list.items[0].ok);
        assert!(!list.items[1].ok);
        assert_eq!(list.skipped.len(), 1);
        assert_eq!(list.skipped[0].index, 1);
    }

    /// **本文が配列でなければ従来どおり失敗すること。**
    ///
    /// 「0件+N件スキップ」と「応答の形が根本的に違う」を混同してはいけない。
    /// 後者は認証エラーの本文やプロキシのHTMLかもしれず、空の一覧として
    /// 扱うと問題が見えなくなる。
    #[tokio::test]
    async fn get_list_fails_when_the_body_is_not_an_array() {
        for body in [
            r#"{"errors":[{"message":"x","code":11}]}"#,
            r#""just a string""#,
            "not json at all",
            "",
        ] {
            let server = MockServer::start().await;
            Mock::given(method("GET"))
                .and(path("/api/v2/things"))
                .respond_with(ResponseTemplate::new(200).set_body_raw(body, "application/json"))
                .mount(&server)
                .await;

            let client = BacklogClient::new(&server.uri(), "KEY").unwrap();
            let result: Result<crate::PartialList<Pong>> =
                client.get_list("/api/v2/things", QueryPairs::new()).await;
            assert!(result.is_err(), "配列でない本文が通ってしまった: {body}");
        }
    }

    /// APIキーとクエリが `get` と同じように付くこと。
    #[tokio::test]
    async fn get_list_sends_the_api_key_header_and_query() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/api/v2/things"))
            .and(header("Backlog-API-Key", "KEY"))
            .and(query_param("count", "50"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!([])))
            .expect(1)
            .mount(&server)
            .await;

        let client = BacklogClient::new(&server.uri(), "KEY").unwrap();
        let mut q = QueryPairs::new();
        q.push("count", "50");
        let _: crate::PartialList<Pong> = client.get_list("/api/v2/things", q).await.unwrap();
    }

    /// 非2xx は `get_list` に入る前に従来どおりエラーになること。
    #[tokio::test]
    async fn get_list_propagates_http_errors() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/api/v2/things"))
            .respond_with(ResponseTemplate::new(401).set_body_json(json!({
                "errors": [{ "message": "Authentication failure.", "code": 11 }]
            })))
            .mount(&server)
            .await;

        let client = BacklogClient::new(&server.uri(), "KEY").unwrap();
        let result: Result<crate::PartialList<Pong>> =
            client.get_list("/api/v2/things", QueryPairs::new()).await;
        assert!(matches!(result, Err(crate::Error::Api { status: 401, .. })));
    }

    fn scratch_dir(tag: &str) -> std::path::PathBuf {
        let dir = std::env::temp_dir().join(format!("backlog-api-rs-test-{tag}"));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[tokio::test]
    async fn start_download_exposes_headers_before_reading_body() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/api/v2/files/1"))
            .and(header("Backlog-API-Key", "KEY"))
            .respond_with(
                ResponseTemplate::new(200)
                    .set_body_bytes(vec![0x89, 0x50, 0x4e, 0x47])
                    .insert_header("content-type", "image/png")
                    .insert_header(
                        "content-disposition",
                        "attachment; filename*=UTF-8''%E5%9B%B3%E9%9D%A2.png",
                    ),
            )
            .expect(1)
            .mount(&server)
            .await;

        let client = BacklogClient::new(&server.uri(), "KEY").unwrap();
        let download = client.start_download("/api/v2/files/1").await.unwrap();
        assert_eq!(download.content_type.as_deref(), Some("image/png"));
        assert_eq!(download.file_name.as_deref(), Some("図面.png"));
        assert_eq!(download.content_length, Some(4));
    }

    #[tokio::test]
    async fn save_to_writes_file_and_returns_size() {
        let dir = scratch_dir("save-ok");
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/api/v2/files/1"))
            .respond_with(ResponseTemplate::new(200).set_body_bytes(b"hello world".to_vec()))
            .mount(&server)
            .await;

        let client = BacklogClient::new(&server.uri(), "KEY").unwrap();
        let download = client.start_download("/api/v2/files/1").await.unwrap();
        let target = dir.join("out.txt");
        let saved = download.save_to(&target, None).await.unwrap();
        assert_eq!(saved.bytes, 11);
        assert_eq!(std::fs::read_to_string(&target).unwrap(), "hello world");
        // 一時ファイルが残っていないこと
        let leftovers: Vec<_> = std::fs::read_dir(&dir)
            .unwrap()
            .map(|e| e.unwrap().file_name().to_string_lossy().to_string())
            .filter(|n| n.ends_with(".part"))
            .collect();
        assert!(leftovers.is_empty(), "leftover part files: {leftovers:?}");
    }

    /// 印が最終パスのファイルに実際に付いていることを実コードパスで確認する。
    ///
    /// 検証しているのは「書き込みから最終パスに現れるまでの間に印が失われない」
    /// ことだけ。`save_to` が返った後にファイルシステムを観察しても、
    /// 「リネームの前に印を付けた」のか「リネームの後に印を付けた」のかは
    /// 区別できない――どちらでも最終的に残るファイルは同じく印付きになるため、
    /// このテストは**リネームの前後関係そのものは検証していない**。
    /// (実際、macOS の `std::fs::copy` は拡張属性を保持するため、リネームを
    /// copy + delete に置き換えても本テストは通り続ける。)
    /// 「リネームの前に印を付ける」という順序は、無印のファイルが利用者に
    /// 見える瞬間をなくすためのものであり、その保証はコードレビューに拠っている。
    #[cfg(target_os = "macos")]
    #[tokio::test]
    async fn save_to_marks_the_final_file_as_internet_origin() {
        let dir = scratch_dir("save-marked");
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/api/v2/files/1"))
            .respond_with(ResponseTemplate::new(200).set_body_bytes(b"payload".to_vec()))
            .mount(&server)
            .await;

        let client = BacklogClient::new(&server.uri(), "KEY").unwrap();
        let download = client.start_download("/api/v2/files/1").await.unwrap();
        let target = dir.join("marked.bin");
        let saved = download.save_to(&target, None).await.unwrap();

        assert_eq!(saved.origin_mark, crate::OriginMark::Applied);
        let value = xattr::get(&target, "com.apple.quarantine")
            .unwrap()
            .expect("最終ファイルに quarantine 属性がない(リネームで失われた可能性)");
        assert!(
            String::from_utf8(value).unwrap().starts_with("0001;"),
            "unexpected quarantine value"
        );
    }

    /// 同じ添付ファイルを再ダウンロードすると、一時ファイルは**既存の最終ファイルを
    /// 上書きする形でリネームされる**。この置き換えリネームの後でも印が
    /// 付いていることを確認する(新規作成のリネームとは異なる、実際に起こりうる
    /// シナリオ)。
    #[cfg(target_os = "macos")]
    #[tokio::test]
    async fn save_to_marks_the_file_after_replacing_an_existing_destination() {
        let dir = scratch_dir("save-marked-replace");
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/api/v2/files/1"))
            .respond_with(ResponseTemplate::new(200).set_body_bytes(b"new payload".to_vec()))
            .mount(&server)
            .await;

        let target = dir.join("marked.bin");
        // 既存の(無印の)ファイルをあらかじめ置いておく。
        std::fs::write(&target, b"old payload").unwrap();
        assert!(
            xattr::get(&target, "com.apple.quarantine")
                .unwrap()
                .is_none()
        );

        let client = BacklogClient::new(&server.uri(), "KEY").unwrap();
        let download = client.start_download("/api/v2/files/1").await.unwrap();
        let saved = download.save_to(&target, None).await.unwrap();

        assert_eq!(saved.origin_mark, crate::OriginMark::Applied);
        assert_eq!(std::fs::read(&target).unwrap(), b"new payload");
        let value = xattr::get(&target, "com.apple.quarantine")
            .unwrap()
            .expect("置き換えリネーム後の最終ファイルに quarantine 属性がない");
        assert!(
            String::from_utf8(value).unwrap().starts_with("0001;"),
            "unexpected quarantine value"
        );
    }

    #[tokio::test]
    async fn save_to_rejects_before_transfer_when_content_length_exceeds_limit() {
        // 注意(実装からの逸脱、レポート参照): ブリーフ原案は「Content-Length は大きいと
        // 宣言するがボディは小さい」レスポンスを wiremock で用意する想定だった。だが
        // wiremock/hyper のサーバー側エンコーダは、ユーザーが指定した Content-Length ヘッダーが
        // 実際のボディ長(Full<Bytes> の既知サイズ)と一致しないと `debug_assert!` で
        // パニックする(hyper 1.11.0 proto/h1/role.rs)。そのため「嘘の Content-Length」を
        // 実際のHTTP応答として作ること自体ができない。
        //
        // 代わりに、実際のボディ長と一致する(嘘のない)大きめの Content-Length を用意し、
        // 書き込み先ディレクトリを存在しないパスにすることで「転送前チェック」を検証する。
        // 事前チェックが効いていれば、ファイルを一切開かずに TooLarge を返すため、
        // 存在しないディレクトリでもエラーは `Io` ではなく `TooLarge` になる。
        // 事前チェックが無ければ、`tokio::fs::File::create` が存在しないディレクトリで
        // 失敗し `Error::Io` になる(あるいは書き込めてもボディの中身に依存した
        // 別のサイズになる)ため、経路の違いを確実に判別できる。
        let server = MockServer::start().await;
        let body = vec![0u8; 65536];
        Mock::given(method("GET"))
            .and(path("/api/v2/files/1"))
            .respond_with(ResponseTemplate::new(200).set_body_bytes(body))
            .mount(&server)
            .await;

        let client = BacklogClient::new(&server.uri(), "KEY").unwrap();
        let download = client.start_download("/api/v2/files/1").await.unwrap();
        assert_eq!(download.content_length, Some(65536));

        let missing_dir = std::env::temp_dir().join("backlog-api-rs-test-save-cl-missing");
        let _ = std::fs::remove_dir_all(&missing_dir);
        let target = missing_dir.join("out.bin");
        let err = download.save_to(&target, Some(1024)).await.unwrap_err();
        match err {
            Error::TooLarge { size, limit } => {
                assert_eq!(size, 65536);
                assert_eq!(limit, 1024);
            }
            other => panic!("expected TooLarge (pre-transfer check), got {other:?}"),
        }
        assert!(!target.exists(), "target should not be created");
        assert!(!missing_dir.exists(), "no directory should be created");
    }

    #[tokio::test]
    async fn save_to_stops_when_body_exceeds_limit_without_content_length() {
        let dir = scratch_dir("save-stream");
        let server = MockServer::start().await;
        // Content-Length を宣言せず、上限より大きいボディを返す。
        // 累積カウンタで止まらなければ最後まで書かれてしまう。
        let body = vec![0u8; 8 * 1024];
        Mock::given(method("GET"))
            .and(path("/api/v2/files/1"))
            .respond_with(
                ResponseTemplate::new(200)
                    .set_body_bytes(body)
                    .insert_header("transfer-encoding", "chunked"),
            )
            .mount(&server)
            .await;

        let client = BacklogClient::new(&server.uri(), "KEY").unwrap();
        let download = client.start_download("/api/v2/files/1").await.unwrap();
        // 検証済み(レポート参照): wiremock は transfer-encoding: chunked を尊重し、
        // Content-Length ヘッダーを付けない。よってこのテストは実際に累積カウンタの
        // 経路を通る(事前チェックはスキップされる)。
        assert_eq!(download.content_length, None);
        let target = dir.join("out.bin");
        let err = download.save_to(&target, Some(1024)).await.unwrap_err();
        match err {
            Error::TooLarge { size, limit } => {
                assert_eq!(limit, 1024);
                // カウンタ経路が実際に働いていることの決定的な証拠: 事前チェック経路
                // (Content-Length ベース)なら size はボディ全長(8192)になるはずだが、
                // カウンタ経路は上限を超えた時点(ストリームの途中)で打ち切るため、
                // size はボディ全長より小さくなる。
                assert!(
                    size < 8192,
                    "size {size} should be less than the full body (8192), \
                     proving the accumulating counter stopped mid-stream rather \
                     than reading to the end"
                );
                assert!(size > 1024, "size {size} should exceed the limit");
            }
            other => panic!("expected TooLarge, got {other:?}"),
        }
        assert!(!target.exists(), "target should not be created");
        assert!(
            std::fs::read_dir(&dir).unwrap().next().is_none(),
            "no partial file should remain"
        );
    }

    #[tokio::test]
    async fn start_download_maps_api_error() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/api/v2/files/9"))
            .respond_with(ResponseTemplate::new(404).set_body_json(serde_json::json!({
                "errors": [{ "message": "No attachment.", "code": 6 }]
            })))
            .mount(&server)
            .await;

        let client = BacklogClient::new(&server.uri(), "KEY").unwrap();
        let err = client.start_download("/api/v2/files/9").await.unwrap_err();
        match err {
            Error::Api { status, errors } => {
                assert_eq!(status, 404);
                assert_eq!(errors[0].message, "No attachment.");
            }
            other => panic!("expected Error::Api, got {other:?}"),
        }
    }

    #[test]
    fn invalid_base_url_is_error() {
        let result = BacklogClient::new("not a url", "KEY");
        assert!(matches!(result, Err(crate::Error::Url(_))));
    }

    #[test]
    fn debug_output_does_not_leak_api_key() {
        let client = BacklogClient::new("https://example.backlog.com", "SUPER_SECRET").unwrap();
        let debug = format!("{client:?}");
        assert!(!debug.contains("SUPER_SECRET"), "got: {debug}");
    }

    #[test]
    fn api_path_passes_plain_segments_through() {
        assert_eq!(api_path(&["issues", "BLG-1"]), "/api/v2/issues/BLG-1");
    }

    #[test]
    fn api_path_encodes_embedded_slash() {
        assert_eq!(api_path(&["issues", "a/b"]), "/api/v2/issues/a%2Fb");
    }

    #[test]
    fn api_path_encodes_embedded_query_string() {
        assert_eq!(
            api_path(&["issues", "BLG-1?archived=false"]),
            "/api/v2/issues/BLG-1%3Farchived=false"
        );
    }

    #[test]
    fn api_path_encodes_non_ascii() {
        assert_eq!(
            api_path(&["issues", "日本語"]),
            "/api/v2/issues/%E6%97%A5%E6%9C%AC%E8%AA%9E"
        );
    }

    /// `url` crate は素の `..` を push() した時点でドットセグメント正規化を
    /// 行い、直前のセグメントを消費してしまう(実験で確認済み)。
    /// api_path() はこれを回避し、直前のセグメントを残したまま安全な
    /// (実在しない)セグメントへと変換しなければならない。
    #[test]
    fn api_path_neutralizes_dot_segment_traversal() {
        let path = api_path(&["issues", ".."]);
        // 直前の "issues" セグメントが消えていないこと。
        assert!(
            path.starts_with("/api/v2/issues/"),
            "dot segment escaped its slot: {path}"
        );
        // 素の ".." がそのまま残っていないこと(正規化されるリスクがある)。
        assert!(!path.ends_with("/.."), "bare .. survived unescaped: {path}");

        // 実際に base_url.join() を通しても "issues" セグメントが残る
        // (別エンドポイントに解決されない)ことを確認する。
        let base = Url::parse("https://example.backlog.com").unwrap();
        let joined = base.join(&path).unwrap();
        assert!(
            joined.path().starts_with("/api/v2/issues/"),
            "joined path escaped issues/: {}",
            joined.path()
        );
    }

    #[test]
    fn temp_path_for_is_unique_per_call() {
        let target = std::path::PathBuf::from("/tmp/example-attachment.png");
        let a = crate::download::temp_path_for(&target);
        let b = crate::download::temp_path_for(&target);
        assert_ne!(a, b, "consecutive temp paths must differ: {a:?} vs {b:?}");
        assert!(
            a.to_string_lossy().ends_with(".part"),
            "expected .part suffix, got {a:?}"
        );
        assert!(
            b.to_string_lossy().ends_with(".part"),
            "expected .part suffix, got {b:?}"
        );
    }

    #[tokio::test]
    async fn save_to_cleans_up_temp_file_when_future_is_dropped_on_cancellation() {
        // MCPクライアントの切断などでFutureが最後まで実行されずdropされるケースを
        // 模擬する。
        //
        // 注意(実装からの逸脱、レポート参照): wiremockの `set_delay` はレスポンス
        // 全体(ヘッダーを含む)の送出を遅らせるため、`start_download().await` の
        // 完了を待ってから `save_to` だけを短いタイムアウトで包んでも、その時点で
        // ボディはすでに全部届いている(=一時ファイルへの書き込みが一瞬で終わり、
        // キャンセルする隙がない)ため使えない。代わりに生のTCPサーバーを使い、
        // ヘッダーと先頭バイトはすぐ送って一時ファイルを実際に作らせ、残りのバイトは
        // 意図的に止めてから、その「ストリーミング中」にFutureをdropさせる。
        let dir = scratch_dir("save-cancel");

        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();

        tokio::spawn(async move {
            use tokio::io::{AsyncReadExt, AsyncWriteExt};
            if let Ok((mut socket, _)) = listener.accept().await {
                let mut buf = [0u8; 1024];
                let _ = socket.read(&mut buf).await;
                let header = b"HTTP/1.1 200 OK\r\nContent-Length: 11\r\nConnection: close\r\n\r\n";
                let _ = socket.write_all(header).await;
                let _ = socket.write_all(b"hello").await;
                let _ = socket.flush().await;
                // ここで長時間止める: 呼び出し側の短いタイムアウトが
                // 「ボディをストリーミング中」に発火するようにする。
                tokio::time::sleep(std::time::Duration::from_secs(2)).await;
                let _ = socket.write_all(b" world").await;
                let _ = socket.shutdown().await;
            }
        });

        let client = BacklogClient::new(&format!("http://{addr}"), "KEY").unwrap();
        let download = client.start_download("/api/v2/files/1").await.unwrap();
        assert_eq!(download.content_length, Some(11));
        let target = dir.join("out.txt");

        let result = tokio::time::timeout(
            std::time::Duration::from_millis(200),
            download.save_to(&target, None),
        )
        .await;
        assert!(
            result.is_err(),
            "expected the save_to future to be cancelled by the timeout"
        );

        // ガードのDropが働いていれば、ディレクトリには何も残らない
        // (.partも最終ファイルも無い)。
        let leftovers: Vec<_> = std::fs::read_dir(&dir)
            .unwrap()
            .map(|e| e.unwrap().file_name().to_string_lossy().to_string())
            .collect();
        assert!(
            leftovers.is_empty(),
            "expected no leftover files after cancellation, found: {leftovers:?}"
        );
    }

    #[test]
    fn api_path_neutralizes_single_dot_segment() {
        let path = api_path(&["issues", "."]);
        assert!(
            path.starts_with("/api/v2/issues/"),
            "dot segment escaped its slot: {path}"
        );
    }

    /// **リダイレクトを追うときにAPIキーを外部ホストへ送らないこと。**
    ///
    /// APIキーは独自ヘッダ `Backlog-API-Key` で送る。reqwest の自動追従は
    /// 独自ヘッダを次のホップへそのまま運ぶ(剥がすのは `Authorization` や
    /// `Cookie` など既知のものだけ)ため、任せると外部ホストにキーが渡る。
    /// [`BacklogClient::send`] で自分で追い、宛先がベースURLと同一オリジンの
    /// ときだけキーを載せている。
    ///
    /// 机上の懸念ではない。`GET /users/{id}/icon` は、アイコンがNulabアカウント側に
    /// ある場合にサーバーが**外部URLへ 302** を返す(サーバー実装
    /// `space/Users.scala` の `NulabAccountIconResource`)。この経路を追うのは
    /// 正常な動作なので、リダイレクト追従そのものは止められない。
    ///
    /// このプロジェクトでAPIキー漏洩を塞ぐのは3例目
    /// (1: `Error::Http` のURL、2: `Auth` の `Debug`、3: これ)。
    #[tokio::test]
    async fn redirect_does_not_leak_api_key_to_the_target_host() {
        // 外部ホストに見立てたサーバー
        let external = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/external/icon.png"))
            .respond_with(ResponseTemplate::new(200).set_body_bytes(b"IMG".to_vec()))
            .expect(1)
            .mount(&external)
            .await;

        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/api/v2/issues/TEST-1/attachments/1"))
            .respond_with(ResponseTemplate::new(302).insert_header(
                "location",
                format!("{}/external/icon.png", external.uri()).as_str(),
            ))
            .expect(1)
            .mount(&server)
            .await;

        let client = BacklogClient::new(&server.uri(), "SUPER_SECRET_KEY").unwrap();
        // リダイレクトは追えること(アイコン取得に必要な正常経路)
        let download = client
            .issues()
            .start_attachment_download("TEST-1", 1)
            .await
            .expect("リダイレクトを追って取得できるべき");
        assert_eq!(download.content_length, Some(3));

        // 外部ホストに届いたリクエストのどこにもキーが無いこと
        let requests = external.received_requests().await.unwrap();
        let request = requests
            .first()
            .expect("外部ホストにリクエストが届いていない");
        assert!(
            request
                .url
                .query()
                .is_none_or(|q| !q.contains("SUPER_SECRET_KEY")),
            "クエリにAPIキーが漏れている: {:?}",
            request.url.query()
        );
        for (name, value) in request.headers.iter() {
            let text = value.to_str().unwrap_or_default();
            assert!(
                !text.contains("SUPER_SECRET_KEY"),
                "ヘッダ {name} にAPIキーが漏れている: {text}"
            );
        }
    }

    /// **同一オリジンへのリダイレクトではAPIキーを載せ続けること。**
    ///
    /// 外部ホストへ渡さない対策は「載せない」方向の判定なので、締めすぎても
    /// テストは緑のままになりうる。追った先が同じスペースなら認証が要る
    /// (キーを落とせば 401 になる)ことを、こちら側から固定しておく。
    #[tokio::test]
    async fn redirect_within_the_same_origin_keeps_the_api_key() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/api/v2/files/moved"))
            .respond_with(ResponseTemplate::new(302).insert_header("location", "/api/v2/files/1"))
            .expect(1)
            .mount(&server)
            .await;
        Mock::given(method("GET"))
            .and(path("/api/v2/files/1"))
            .and(header("Backlog-API-Key", "KEY"))
            .respond_with(ResponseTemplate::new(200).set_body_bytes(b"IMG".to_vec()))
            .expect(1)
            .mount(&server)
            .await;

        let client = BacklogClient::new(&server.uri(), "KEY").unwrap();
        let download = client.start_download("/api/v2/files/moved").await.unwrap();
        assert_eq!(download.content_length, Some(3));
    }

    /// **APIキーがクエリ文字列に一切載らないこと。**
    ///
    /// ヘッダに移した目的は、クエリ文字列が辿る経路(アクセスログ・`Referer`・
    /// エラーメッセージのURL)にキーを載せないことにある。ヘッダが付いている
    /// ことだけを見るテストは、クエリにも二重に載る退行を通してしまう。
    #[tokio::test]
    async fn the_api_key_never_appears_in_the_query_string() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/api/v2/ping"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({ "ok": true })))
            .expect(1)
            .mount(&server)
            .await;

        let client = BacklogClient::new(&server.uri(), "SUPER_SECRET_KEY").unwrap();
        let mut query = QueryPairs::new();
        query.push("count", "10");
        let _: Pong = client.get("/api/v2/ping", query).await.unwrap();

        let requests = server.received_requests().await.unwrap();
        let url = &requests.first().expect("リクエストが届いていない").url;
        assert!(
            !url.as_str().contains("SUPER_SECRET_KEY"),
            "URLにAPIキーが漏れている: {url}"
        );
    }

    /// リダイレクトのループを上限で止めること。
    ///
    /// 追従を reqwest から引き取った以上、ループを止める責任もこちらにある。
    #[tokio::test]
    async fn redirect_loops_stop_at_the_limit() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/api/v2/loop"))
            .respond_with(ResponseTemplate::new(302).insert_header("location", "/api/v2/loop"))
            .mount(&server)
            .await;

        let client = BacklogClient::new(&server.uri(), "KEY").unwrap();
        let err = client.start_download("/api/v2/loop").await.unwrap_err();
        assert!(
            matches!(err, Error::TooManyRedirects { .. }),
            "想定外のエラー: {err:?}"
        );
    }

    /// reqwest::Error の Display/Debug にはリクエストURLが含まれる。APIキーは
    /// ヘッダで送るのでURLには載らないが、`without_url()` は多層防御として
    /// 残してある。その層が生きていることを固定する。unroutableなアドレスへの
    /// 接続を試みて、送信エラーになったときのメッセージにAPIキーが含まれないことを
    /// 確認する。
    #[tokio::test]
    async fn http_error_does_not_leak_api_key() {
        let client = BacklogClient::new("http://127.0.0.1:1", "SUPER_SECRET").unwrap();
        let err = client
            .get::<Pong>("/api/v2/ping", QueryPairs::new())
            .await
            .unwrap_err();
        let display = err.to_string();
        let debug = format!("{err:?}");
        assert!(
            !display.contains("SUPER_SECRET"),
            "Display leaked api key: {display}"
        );
        assert!(
            !debug.contains("SUPER_SECRET"),
            "Debug leaked api key: {debug}"
        );
    }
}
