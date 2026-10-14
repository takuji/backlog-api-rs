use crate::models::User;
use crate::params::QueryPairs;
use crate::{BacklogClient, Result};

/// ユーザー関連API。`client.users()` で取得する。
#[derive(Debug, Clone, Copy)]
pub struct UsersApi<'a> {
    client: &'a BacklogClient,
}

impl<'a> UsersApi<'a> {
    pub(crate) fn new(client: &'a BacklogClient) -> Self {
        Self { client }
    }

    /// APIキーのユーザー自身の情報を取得する。
    pub async fn myself(&self) -> Result<User> {
        self.client
            .get("/api/v2/users/myself", QueryPairs::new())
            .await
    }

    /// スペースのユーザー一覧を取得する。
    pub async fn list(&self) -> Result<Vec<User>> {
        self.client.get("/api/v2/users", QueryPairs::new()).await
    }

    /// ユーザーのアイコン画像を取得する。
    ///
    /// 返るのは画像そのもの(JSONではない)。小さいので
    /// [`crate::AttachmentDownload::read_to_vec`] でメモリに読むのが普通。
    ///
    /// **このエンドポイントは外部URLへ 302 リダイレクトすることがある。**
    /// アイコンの実体が Nulab アカウント側にある場合、サーバーは
    /// そのURLへリダイレクトする(サーバー実装 `space/Users.scala` の
    /// `NulabAccountIconResource`)。クライアントはリダイレクトを追うが、
    /// APIキーをクエリで送っている都合上 `Referer` を切ってある
    /// (切らないとリダイレクト先の外部ホストにキーが渡る。`client.rs` 参照)。
    ///
    /// `Content-Type` が `application/octet-stream` で返ることがある。
    /// 画像として扱うには中身のマジックバイトから判定する必要がある場合がある。
    pub async fn icon(&self, user_id: u64) -> Result<crate::AttachmentDownload> {
        self.client
            .start_download(&crate::client::api_path(&[
                "users",
                &user_id.to_string(),
                "icon",
            ]))
            .await
    }
}

#[cfg(test)]
mod tests {
    use crate::BacklogClient;
    use serde_json::json;
    use wiremock::matchers::{header, method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    fn user_json() -> serde_json::Value {
        json!({
            "id": 1, "userId": "admin", "name": "admin", "roleType": 1,
            "lang": "ja", "mailAddress": "admin@nulab.example"
        })
    }

    #[tokio::test]
    async fn myself_fetches_own_user() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/api/v2/users/myself"))
            .and(header("Backlog-API-Key", "KEY"))
            .respond_with(ResponseTemplate::new(200).set_body_json(user_json()))
            .expect(1)
            .mount(&server)
            .await;

        let client = BacklogClient::new(&server.uri(), "KEY").unwrap();
        let me = client.users().myself().await.unwrap();
        assert_eq!(me.user_id.as_deref(), Some("admin"));
    }

    #[tokio::test]
    async fn list_fetches_all_users() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/api/v2/users"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!([user_json()])))
            .mount(&server)
            .await;

        let client = BacklogClient::new(&server.uri(), "KEY").unwrap();
        let users = client.users().list().await.unwrap();
        assert_eq!(users.len(), 1);
    }

    /// アイコンが正しいパスを叩き、バイト列として読めること。
    ///
    /// JSONではなく画像が返るエンドポイントなので、`get` ではなく
    /// ダウンロード経路を通る必要がある。
    #[tokio::test]
    async fn icon_downloads_the_image_bytes() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/api/v2/users/1234/icon"))
            .respond_with(
                ResponseTemplate::new(200)
                    .insert_header("content-type", "image/png")
                    .set_body_bytes(b"\x89PNG-body".to_vec()),
            )
            .expect(1)
            .mount(&server)
            .await;

        let client = BacklogClient::new(&server.uri(), "KEY").unwrap();
        let download = client.users().icon(1234).await.unwrap();
        assert_eq!(download.content_type.as_deref(), Some("image/png"));
        let bytes = download.read_to_vec(Some(1024)).await.unwrap();
        assert_eq!(bytes, b"\x89PNG-body");
    }

    /// アイコンが外部URLへ 302 した場合でも追従して取得できること。
    ///
    /// 実装(`space/Users.scala`)は、アイコンの実体がNulabアカウント側に
    /// ある場合に外部URLへリダイレクトする。**この経路は正常動作なので
    /// 追従を止めてはならない。** APIキーが漏れないことは `client.rs` の
    /// `redirect_does_not_leak_api_key_to_the_target_host` が担保する。
    #[tokio::test]
    async fn icon_follows_a_redirect_to_an_external_host() {
        let external = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/nulab/icon.png"))
            .respond_with(ResponseTemplate::new(200).set_body_bytes(b"EXTERNAL".to_vec()))
            .expect(1)
            .mount(&external)
            .await;

        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/api/v2/users/1234/icon"))
            .respond_with(ResponseTemplate::new(302).insert_header(
                "location",
                format!("{}/nulab/icon.png", external.uri()).as_str(),
            ))
            .expect(1)
            .mount(&server)
            .await;

        let client = BacklogClient::new(&server.uri(), "KEY").unwrap();
        let bytes = client
            .users()
            .icon(1234)
            .await
            .unwrap()
            .read_to_vec(Some(1024))
            .await
            .unwrap();
        assert_eq!(bytes, b"EXTERNAL");
    }

    /// `Content-Length` が無い場合でも、累積カウンタで上限に達したら止まること。
    ///
    /// 上の「転送前に弾かれる」テストは `Content-Length` 付きの応答なので
    /// **事前チェックしか通らず、累積カウンタは検証できない**(実際、累積側の
    /// 判定を無効化する変異が生き残ることを確認して、このテストを足した)。
    /// `transfer-encoding: chunked` を付けると wiremock は `Content-Length` を
    /// 付けないので、事前チェックが飛ばされてカウンタ経路に入る。
    #[tokio::test]
    async fn icon_stops_when_body_exceeds_limit_without_content_length() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/api/v2/users/1234/icon"))
            .respond_with(
                ResponseTemplate::new(200)
                    .set_body_bytes(vec![0u8; 8 * 1024])
                    .insert_header("transfer-encoding", "chunked"),
            )
            .mount(&server)
            .await;

        let client = BacklogClient::new(&server.uri(), "KEY").unwrap();
        let download = client.users().icon(1234).await.unwrap();
        // 事前チェックが効かないことの確認(効いていたらこのテストの意味がない)
        assert_eq!(download.content_length, None);

        let err = download.read_to_vec(Some(1024)).await.unwrap_err();
        match err {
            crate::Error::TooLarge { size, limit } => {
                assert_eq!(limit, 1024);
                // カウンタ経路である決定的な証拠: 事前チェックなら size は
                // ボディ全長(8192)になるが、カウンタは超えた時点で打ち切るため小さい
                assert!(size < 8 * 1024, "全部読んでから判定している: size={size}");
                assert!(size > 1024, "上限を超えた時点の値のはず: size={size}");
            }
            other => panic!("TooLarge を期待したが {other:?}"),
        }
    }

    /// 上限を超えるアイコンは**転送前に**弾かれること。
    ///
    /// `size` が宣言された `Content-Length` そのもの(8192)であることが、
    /// 事前チェックを通った決定的な証拠になる。累積カウンタ経路なら上限を
    /// 超えた時点で打ち切るので `size` はもっと小さくなる。
    /// 本文が小さいと両経路の `size` が一致してしまい区別できない。
    #[tokio::test]
    async fn icon_rejects_an_oversized_image_before_transferring() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/api/v2/users/1234/icon"))
            .respond_with(ResponseTemplate::new(200).set_body_bytes(vec![0u8; 8 * 1024]))
            .mount(&server)
            .await;

        let client = BacklogClient::new(&server.uri(), "KEY").unwrap();
        let download = client.users().icon(1234).await.unwrap();
        assert_eq!(download.content_length, Some(8 * 1024));

        let err = download.read_to_vec(Some(1024)).await.unwrap_err();
        match err {
            crate::Error::TooLarge { size, limit } => {
                assert_eq!(limit, 1024);
                assert_eq!(size, 8 * 1024, "事前チェックなら宣言値がそのまま入る");
            }
            other => panic!("TooLarge を期待したが {other:?}"),
        }
    }
}
