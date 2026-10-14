use serde::Deserialize;

use crate::models::{Wiki, WikiAttachment, WikiTag};
use crate::params::QueryPairs;
use crate::{AttachmentDownload, BacklogClient, Result};

/// Wiki 関連API。`client.wikis()` で取得する。
#[derive(Debug, Clone, Copy)]
pub struct WikisApi<'a> {
    client: &'a BacklogClient,
}

/// Wiki一覧の取得(GET /api/v2/wikis)のパラメータ。
///
/// <https://developer.nulab.com/docs/backlog/api/2/get-wiki-page-list/>
#[derive(Debug, Clone)]
pub struct GetWikisParams {
    pairs: QueryPairs,
}

impl GetWikisParams {
    /// プロジェクトを指定して作る(必須)。IDまたはプロジェクトキー。
    pub fn new(project_id_or_key: &str) -> Self {
        let mut pairs = QueryPairs::new();
        pairs.push("projectIdOrKey", project_id_or_key);
        Self { pairs }
    }

    /// ページ名の部分一致フィルタ(本文は検索対象外)。
    pub fn keyword(mut self, keyword: &str) -> Self {
        self.pairs.push("keyword", keyword);
        self
    }

    /// ソート対象("created", "updated", "name" など)。
    pub fn sort(mut self, key: &str) -> Self {
        self.pairs.push("sort", key);
        self
    }

    pub(crate) fn into_pairs(self) -> QueryPairs {
        self.pairs
    }
}

/// Wikiページの追加(POST /api/v2/wikis)のパラメータ。
///
/// <https://developer.nulab.com/docs/backlog/api/2/add-wiki-page/>
#[derive(Debug, Clone)]
pub struct AddWikiParams {
    pairs: QueryPairs,
}

impl AddWikiParams {
    pub fn new(project_id: u64, name: &str, content: &str) -> Self {
        let mut pairs = QueryPairs::new();
        pairs.push("projectId", project_id);
        pairs.push("name", name);
        pairs.push("content", content);
        Self { pairs }
    }

    /// メール通知を送るか(既定はAPI側で false)。
    pub fn mail_notify(mut self, value: bool) -> Self {
        self.pairs.push("mailNotify", value);
        self
    }

    pub(crate) fn into_pairs(self) -> QueryPairs {
        self.pairs
    }
}

/// Wikiページの更新(PATCH /api/v2/wikis/{wikiId})のパラメータ。すべて任意。
///
/// <https://developer.nulab.com/docs/backlog/api/2/update-wiki-page/>
#[derive(Debug, Clone, Default)]
pub struct UpdateWikiParams {
    pairs: QueryPairs,
}

impl UpdateWikiParams {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn name(mut self, name: &str) -> Self {
        self.pairs.push("name", name);
        self
    }

    pub fn content(mut self, content: &str) -> Self {
        self.pairs.push("content", content);
        self
    }

    /// メール通知を送るか(既定はAPI側で false)。
    pub fn mail_notify(mut self, value: bool) -> Self {
        self.pairs.push("mailNotify", value);
        self
    }

    pub(crate) fn into_pairs(self) -> QueryPairs {
        self.pairs
    }
}

#[derive(Debug, Deserialize)]
struct CountResponse {
    count: u64,
}

impl<'a> WikisApi<'a> {
    pub(crate) fn new(client: &'a BacklogClient) -> Self {
        Self { client }
    }

    /// Wikiページ一覧を取得する。レスポンスには各ページの本文も含まれる。
    pub async fn list(&self, params: GetWikisParams) -> Result<Vec<Wiki>> {
        self.client.get("/api/v2/wikis", params.into_pairs()).await
    }

    /// プロジェクトのWikiページ数を取得する。
    pub async fn count(&self, project_id_or_key: &str) -> Result<u64> {
        let mut query = QueryPairs::new();
        query.push("projectIdOrKey", project_id_or_key);
        let resp: CountResponse = self.client.get("/api/v2/wikis/count", query).await?;
        Ok(resp.count)
    }

    /// Wikiページを取得する。
    pub async fn get(&self, wiki_id: u64) -> Result<Wiki> {
        self.client
            .get(
                &crate::client::api_path(&["wikis", &wiki_id.to_string()]),
                QueryPairs::new(),
            )
            .await
    }

    /// Wikiページを追加する。
    pub async fn add(&self, params: AddWikiParams) -> Result<Wiki> {
        self.client.post("/api/v2/wikis", params.into_pairs()).await
    }

    /// Wikiページを更新する。指定したフィールドのみ変更される。
    pub async fn update(&self, wiki_id: u64, params: UpdateWikiParams) -> Result<Wiki> {
        self.client
            .patch(
                &crate::client::api_path(&["wikis", &wiki_id.to_string()]),
                params.into_pairs(),
            )
            .await
    }

    /// プロジェクトで使われているWikiタグの一覧を取得する。
    pub async fn tags(&self, project_id_or_key: &str) -> Result<Vec<WikiTag>> {
        let mut query = QueryPairs::new();
        query.push("projectIdOrKey", project_id_or_key);
        self.client.get("/api/v2/wikis/tags", query).await
    }

    /// Wikiページの添付ファイル一覧を取得する。
    pub async fn attachments(&self, wiki_id: u64) -> Result<Vec<WikiAttachment>> {
        self.client
            .get(
                &crate::client::api_path(&["wikis", &wiki_id.to_string(), "attachments"]),
                QueryPairs::new(),
            )
            .await
    }

    /// Wikiページの添付ファイルのダウンロードを開始する。
    ///
    /// 返ったハンドルの [`AttachmentDownload::save_to`] でファイルに書き出す。
    pub async fn start_attachment_download(
        &self,
        wiki_id: u64,
        attachment_id: u64,
    ) -> Result<AttachmentDownload> {
        self.client
            .start_download(&crate::client::api_path(&[
                "wikis",
                &wiki_id.to_string(),
                "attachments",
                &attachment_id.to_string(),
            ]))
            .await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::BacklogClient;
    use serde_json::json;
    use wiremock::matchers::{body_string_contains, method, path, query_param};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    fn wiki_json() -> serde_json::Value {
        json!({
            "id": 1, "projectId": 7, "name": "Home",
            "content": "本文です",
            "tags": [{ "id": 12, "name": "議事録" }],
            "attachments": [], "sharedFiles": [], "stars": [],
            "createdUser": { "id": 1, "userId": "admin", "name": "admin", "roleType": 1 },
            "created": "2026-07-01T06:10:15Z",
            "updatedUser": null, "updated": null
        })
    }

    #[test]
    fn get_wikis_params_builds_query_pairs() {
        let params = GetWikisParams::new("TEST1").keyword("設計").sort("updated");
        let pairs = params.into_pairs();
        let slice = pairs.as_slice();
        assert!(slice.contains(&("projectIdOrKey".to_string(), "TEST1".to_string())));
        assert!(slice.contains(&("keyword".to_string(), "設計".to_string())));
        assert!(slice.contains(&("sort".to_string(), "updated".to_string())));
    }

    #[test]
    fn add_wiki_params_builds_form_pairs() {
        let params = AddWikiParams::new(7, "新規ページ", "本文").mail_notify(true);
        let pairs = params.into_pairs();
        let slice = pairs.as_slice();
        assert!(slice.contains(&("projectId".to_string(), "7".to_string())));
        assert!(slice.contains(&("name".to_string(), "新規ページ".to_string())));
        assert!(slice.contains(&("content".to_string(), "本文".to_string())));
        assert!(slice.contains(&("mailNotify".to_string(), "true".to_string())));
    }

    #[test]
    fn update_wiki_params_only_sets_given_fields() {
        let params = UpdateWikiParams::new().name("改題");
        let pairs = params.into_pairs();
        let slice = pairs.as_slice();
        assert!(slice.contains(&("name".to_string(), "改題".to_string())));
        assert!(!slice.iter().any(|(k, _)| k == "content"));
    }

    #[tokio::test]
    async fn list_fetches_wikis_with_params() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/api/v2/wikis"))
            .and(query_param("projectIdOrKey", "TEST1"))
            .and(query_param("keyword", "設計"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!([wiki_json()])))
            .expect(1)
            .mount(&server)
            .await;

        let client = BacklogClient::new(&server.uri(), "KEY").unwrap();
        let wikis = client
            .wikis()
            .list(GetWikisParams::new("TEST1").keyword("設計"))
            .await
            .unwrap();
        assert_eq!(wikis[0].name, "Home");
    }

    #[tokio::test]
    async fn count_unwraps_count_field() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/api/v2/wikis/count"))
            .and(query_param("projectIdOrKey", "TEST1"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({ "count": 12 })))
            .mount(&server)
            .await;

        let client = BacklogClient::new(&server.uri(), "KEY").unwrap();
        assert_eq!(client.wikis().count("TEST1").await.unwrap(), 12);
    }

    #[tokio::test]
    async fn get_fetches_single_wiki() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/api/v2/wikis/1"))
            .respond_with(ResponseTemplate::new(200).set_body_json(wiki_json()))
            .mount(&server)
            .await;

        let client = BacklogClient::new(&server.uri(), "KEY").unwrap();
        let wiki = client.wikis().get(1).await.unwrap();
        assert_eq!(wiki.content.as_deref(), Some("本文です"));
    }

    #[tokio::test]
    async fn add_posts_form_and_returns_wiki() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/api/v2/wikis"))
            .and(body_string_contains("projectId=7"))
            .respond_with(ResponseTemplate::new(201).set_body_json(wiki_json()))
            .expect(1)
            .mount(&server)
            .await;

        let client = BacklogClient::new(&server.uri(), "KEY").unwrap();
        let wiki = client
            .wikis()
            .add(AddWikiParams::new(7, "Home", "本文です"))
            .await
            .unwrap();
        assert_eq!(wiki.id, 1);
    }

    #[tokio::test]
    async fn update_patches_form() {
        let server = MockServer::start().await;
        Mock::given(method("PATCH"))
            .and(path("/api/v2/wikis/1"))
            .and(body_string_contains("content="))
            .respond_with(ResponseTemplate::new(200).set_body_json(wiki_json()))
            .expect(1)
            .mount(&server)
            .await;

        let client = BacklogClient::new(&server.uri(), "KEY").unwrap();
        let wiki = client
            .wikis()
            .update(1, UpdateWikiParams::new().content("更新後"))
            .await
            .unwrap();
        assert_eq!(wiki.id, 1);
    }

    #[tokio::test]
    async fn tags_fetches_wiki_tags() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/api/v2/wikis/tags"))
            .and(query_param("projectIdOrKey", "TEST1"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!([
                { "id": 12, "name": "議事録" }
            ])))
            .mount(&server)
            .await;

        let client = BacklogClient::new(&server.uri(), "KEY").unwrap();
        let tags = client.wikis().tags("TEST1").await.unwrap();
        assert_eq!(tags[0].name, "議事録");
    }

    #[tokio::test]
    async fn attachments_lists_wiki_attachments() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/api/v2/wikis/1/attachments"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!([
                { "id": 5, "name": "図面.png", "size": 1024 }
            ])))
            .mount(&server)
            .await;

        let client = BacklogClient::new(&server.uri(), "KEY").unwrap();
        let list = client.wikis().attachments(1).await.unwrap();
        assert_eq!(list[0].name, "図面.png");
    }

    #[tokio::test]
    async fn start_attachment_download_fetches_wiki_file() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/api/v2/wikis/1/attachments/5"))
            .respond_with(
                ResponseTemplate::new(200)
                    .set_body_bytes(vec![7, 8])
                    .insert_header("content-type", "image/png"),
            )
            .expect(1)
            .mount(&server)
            .await;

        let client = BacklogClient::new(&server.uri(), "KEY").unwrap();
        let download = client
            .wikis()
            .start_attachment_download(1, 5)
            .await
            .unwrap();
        assert_eq!(download.content_type.as_deref(), Some("image/png"));
        assert_eq!(download.content_length, Some(2));
    }
}
