use crate::models::{Priority, Resolution, Space};
use crate::params::QueryPairs;
use crate::{BacklogClient, Result};

/// スペース全体の参照データAPI。`client.space()` で取得する。
#[derive(Debug, Clone, Copy)]
pub struct SpaceApi<'a> {
    client: &'a BacklogClient,
}

impl<'a> SpaceApi<'a> {
    pub(crate) fn new(client: &'a BacklogClient) -> Self {
        Self { client }
    }

    /// スペースの情報を取得する。
    pub async fn info(&self) -> Result<Space> {
        self.client.get("/api/v2/space", QueryPairs::new()).await
    }

    /// 優先度一覧を取得する。
    /// スペースのロゴ画像を取得する。
    ///
    /// 返るのは画像そのもの(JSONではない)。小さいので
    /// [`crate::AttachmentDownload::read_to_vec`] でメモリに読むのが普通。
    ///
    /// ユーザーアイコンと違い、このエンドポイントはリダイレクトしない。
    /// Nulab アカウント連携時もサーバー側で取得してストリームする
    /// (サーバー実装 `space/Spaces.scala`)。
    ///
    /// `Content-Type` が `application/octet-stream` で返ることがある。
    pub async fn image(&self) -> Result<crate::AttachmentDownload> {
        self.client.start_download("/api/v2/space/image").await
    }

    pub async fn priorities(&self) -> Result<Vec<Priority>> {
        self.client
            .get("/api/v2/priorities", QueryPairs::new())
            .await
    }

    /// 完了理由一覧を取得する。
    pub async fn resolutions(&self) -> Result<Vec<Resolution>> {
        self.client
            .get("/api/v2/resolutions", QueryPairs::new())
            .await
    }
}

#[cfg(test)]
mod tests {
    use crate::BacklogClient;
    use serde_json::json;
    use wiremock::matchers::{method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    /// スペース情報が正しいパスから取得できること。
    #[tokio::test]
    async fn info_requests_the_space_path() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/api/v2/space"))
            .respond_with(ResponseTemplate::new(200).set_body_raw(
                r#"{ "spaceKey": "EXAMPLE", "name": "サンプルスペース", "ownerId": 1,
                     "lang": "ja", "timezone": "Asia/Tokyo" }"#,
                "application/json",
            ))
            .expect(1)
            .mount(&server)
            .await;

        let client = BacklogClient::new(&server.uri(), "KEY").unwrap();
        let space = client.space().info().await.unwrap();
        assert_eq!(space.space_key, "EXAMPLE");
        assert_eq!(space.name, "サンプルスペース");
    }

    #[tokio::test]
    async fn priorities_fetches_list() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/api/v2/priorities"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!([
                { "id": 2, "name": "高" }, { "id": 3, "name": "中" }, { "id": 4, "name": "低" }
            ])))
            .mount(&server)
            .await;

        let client = BacklogClient::new(&server.uri(), "KEY").unwrap();
        let priorities = client.space().priorities().await.unwrap();
        assert_eq!(priorities.len(), 3);
        assert_eq!(priorities[0].name, "高");
    }

    #[tokio::test]
    async fn resolutions_fetches_list() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/api/v2/resolutions"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!([
                { "id": 0, "name": "対応済み" }
            ])))
            .mount(&server)
            .await;

        let client = BacklogClient::new(&server.uri(), "KEY").unwrap();
        let resolutions = client.space().resolutions().await.unwrap();
        assert_eq!(resolutions[0].id, 0);
    }

    /// スペースロゴが正しいパスを叩き、バイト列として読めること。
    #[tokio::test]
    async fn image_downloads_the_logo_bytes() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/api/v2/space/image"))
            .respond_with(
                ResponseTemplate::new(200)
                    // 実APIは application/octet-stream で返すことがある
                    .insert_header("content-type", "application/octet-stream")
                    .set_body_bytes(b"LOGO".to_vec()),
            )
            .expect(1)
            .mount(&server)
            .await;

        let client = BacklogClient::new(&server.uri(), "KEY").unwrap();
        let download = client.space().image().await.unwrap();
        assert_eq!(
            download.content_type.as_deref(),
            Some("application/octet-stream")
        );
        assert_eq!(download.read_to_vec(Some(1024)).await.unwrap(), b"LOGO");
    }
}
