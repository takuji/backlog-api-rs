use serde::Deserialize;

use crate::api::Order;
use crate::models::{Document, DocumentComment, DocumentTree};
use crate::params::QueryPairs;
use crate::{AttachmentDownload, BacklogClient, Result};

/// ドキュメント関連API。`client.documents()` で取得する。
#[derive(Debug, Clone, Copy)]
pub struct DocumentsApi<'a> {
    client: &'a BacklogClient,
}

/// ドキュメント一覧の取得(GET /api/v2/documents)のパラメータ。
#[derive(Debug, Clone, Default)]
pub struct GetDocumentsParams {
    pairs: QueryPairs,
}

impl GetDocumentsParams {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn project_ids(mut self, ids: impl IntoIterator<Item = u64>) -> Self {
        self.pairs.push_array("projectId[]", ids);
        self
    }

    /// 検索キーワード。
    pub fn keyword(mut self, keyword: &str) -> Self {
        self.pairs.push("keyword", keyword);
        self
    }

    /// ソート対象の属性名。
    pub fn sort(mut self, key: &str) -> Self {
        self.pairs.push("sort", key);
        self
    }

    pub fn order(mut self, order: Order) -> Self {
        self.pairs.push("order", order.as_str());
        self
    }

    /// 取得開始位置。**APIの必須パラメータ**。省略すると400になる。
    pub fn offset(mut self, offset: u64) -> Self {
        self.pairs.push("offset", offset);
        self
    }

    /// 取得上限。
    pub fn count(mut self, count: u64) -> Self {
        self.pairs.push("count", count);
        self
    }

    pub(crate) fn into_pairs(self) -> QueryPairs {
        self.pairs
    }
}

/// ドキュメントの追加(POST /api/v2/documents)のパラメータ。
///
/// `content` は Markdown として解釈される。
#[derive(Debug, Clone)]
pub struct AddDocumentParams {
    pairs: QueryPairs,
}

impl AddDocumentParams {
    /// 必須パラメータ(プロジェクトID)を指定して作る。
    pub fn new(project_id: u64) -> Self {
        let mut pairs = QueryPairs::new();
        pairs.push("projectId", project_id);
        Self { pairs }
    }

    pub fn title(mut self, title: &str) -> Self {
        self.pairs.push("title", title);
        self
    }

    /// 本文(Markdownとして解釈される)。
    pub fn content(mut self, content: &str) -> Self {
        self.pairs.push("content", content);
        self
    }

    /// タイトル前に表示する絵文字(1文字)。
    pub fn emoji(mut self, emoji: &str) -> Self {
        self.pairs.push("emoji", emoji);
        self
    }

    /// 指定すると、そのドキュメントの子として作成される。
    pub fn parent_id(mut self, parent_id: &str) -> Self {
        self.pairs.push("parentId", parent_id);
        self
    }

    /// 兄弟ドキュメントの末尾に追加するか。
    pub fn add_last(mut self, value: bool) -> Self {
        self.pairs.push("addLast", value);
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

impl<'a> DocumentsApi<'a> {
    pub(crate) fn new(client: &'a BacklogClient) -> Self {
        Self { client }
    }

    /// ドキュメントを検索する。レスポンスには各ドキュメントの本文も含まれる。
    ///
    /// `offset` はAPIの必須パラメータ。省略すると400になる。
    pub async fn list(&self, params: GetDocumentsParams) -> Result<Vec<Document>> {
        self.client
            .get("/api/v2/documents", params.into_pairs())
            .await
    }

    /// プロジェクトのドキュメント数を取得する。
    pub async fn count(&self, project_id_or_key: &str) -> Result<u64> {
        let mut query = QueryPairs::new();
        query.push("projectIdOrKey", project_id_or_key);
        let resp: CountResponse = self.client.get("/api/v2/documents/count", query).await?;
        Ok(resp.count)
    }

    /// プロジェクトのドキュメントツリーを取得する。
    pub async fn tree(&self, project_id_or_key: &str) -> Result<DocumentTree> {
        let mut query = QueryPairs::new();
        query.push("projectIdOrKey", project_id_or_key);
        self.client.get("/api/v2/documents/tree", query).await
    }

    /// ドキュメントを取得する。
    pub async fn get(&self, document_id: &str) -> Result<Document> {
        self.client
            .get(
                &crate::client::api_path(&["documents", document_id]),
                QueryPairs::new(),
            )
            .await
    }

    /// ドキュメントを追加する。
    pub async fn add(&self, params: AddDocumentParams) -> Result<Document> {
        self.client
            .post("/api/v2/documents", params.into_pairs())
            .await
    }

    /// ドキュメントのコメント一覧を取得する。
    pub async fn comments(&self, document_id: &str) -> Result<Vec<DocumentComment>> {
        self.client
            .get(
                &crate::client::api_path(&["documents", document_id, "comments"]),
                QueryPairs::new(),
            )
            .await
    }

    /// ドキュメントの添付ファイルのダウンロードを開始する。
    ///
    /// 添付ファイルの一覧はこのAPIには無く、`get()` の `attachments` から得る。
    /// 返ったハンドルの [`AttachmentDownload::save_to`] でファイルに書き出す。
    pub async fn start_attachment_download(
        &self,
        document_id: &str,
        attachment_id: &str,
    ) -> Result<AttachmentDownload> {
        self.client
            .start_download(&crate::client::api_path(&[
                "documents",
                document_id,
                "attachments",
                attachment_id,
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

    fn document_json() -> serde_json::Value {
        json!({
            "id": "doc-abc123", "projectId": 7, "title": "設計メモ",
            "json": "{}", "plain": "本文のプレーンテキスト",
            "emoji": "📝", "statusId": 1,
            "attachments": [], "tags": [{ "id": 3, "name": "設計" }],
            "createdUser": { "id": 1, "userId": "admin", "name": "admin", "roleType": 1 },
            "created": "2026-07-01T06:10:15Z",
            "updatedUser": null, "updated": null
        })
    }

    #[test]
    fn get_documents_params_builds_query_pairs() {
        let params = GetDocumentsParams::new()
            .project_ids([7u64, 8])
            .keyword("設計")
            .sort("updated")
            .order(Order::Desc)
            .offset(10)
            .count(50);
        let pairs = params.into_pairs();
        let slice = pairs.as_slice();
        assert!(slice.contains(&("projectId[]".to_string(), "7".to_string())));
        assert!(slice.contains(&("projectId[]".to_string(), "8".to_string())));
        assert!(slice.contains(&("keyword".to_string(), "設計".to_string())));
        assert!(slice.contains(&("sort".to_string(), "updated".to_string())));
        assert!(slice.contains(&("order".to_string(), "desc".to_string())));
        assert!(slice.contains(&("offset".to_string(), "10".to_string())));
        assert!(slice.contains(&("count".to_string(), "50".to_string())));
    }

    #[test]
    fn add_document_params_builds_form_pairs() {
        let params = AddDocumentParams::new(7)
            .title("新規ドキュメント")
            .content("# 見出し")
            .emoji("📝")
            .parent_id("doc-parent")
            .add_last(true);
        let pairs = params.into_pairs();
        let slice = pairs.as_slice();
        assert!(slice.contains(&("projectId".to_string(), "7".to_string())));
        assert!(slice.contains(&("title".to_string(), "新規ドキュメント".to_string())));
        assert!(slice.contains(&("content".to_string(), "# 見出し".to_string())));
        assert!(slice.contains(&("emoji".to_string(), "📝".to_string())));
        assert!(slice.contains(&("parentId".to_string(), "doc-parent".to_string())));
        assert!(slice.contains(&("addLast".to_string(), "true".to_string())));
    }

    #[tokio::test]
    async fn list_fetches_documents_with_params() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/api/v2/documents"))
            .and(query_param("projectId[]", "7"))
            .and(query_param("keyword", "設計"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!([document_json()])))
            .expect(1)
            .mount(&server)
            .await;

        let client = BacklogClient::new(&server.uri(), "KEY").unwrap();
        let docs = client
            .documents()
            .list(
                GetDocumentsParams::new()
                    .project_ids([7u64])
                    .keyword("設計"),
            )
            .await
            .unwrap();
        assert_eq!(docs[0].id, "doc-abc123");
    }

    #[tokio::test]
    async fn count_unwraps_count_field() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/api/v2/documents/count"))
            .and(query_param("projectIdOrKey", "TEST1"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({ "count": 34 })))
            .mount(&server)
            .await;

        let client = BacklogClient::new(&server.uri(), "KEY").unwrap();
        assert_eq!(client.documents().count("TEST1").await.unwrap(), 34);
    }

    #[tokio::test]
    async fn tree_fetches_nested_nodes() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/api/v2/documents/tree"))
            .and(query_param("projectIdOrKey", "TEST1"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "projectId": 1,
                "activeTree": {
                    "id": "Active",
                    "children": [
                        {
                            "id": "01934345404771adb2113d7792bb4351",
                            "name": "local test",
                            "children": [
                                {
                                    "id": "019347fc760c7b0abff04b44628c94d7",
                                    "name": "test2",
                                    "children": [
                                        { "id": "0192ff5990da76c289dee06b1f11fa01", "name": "aaatest234", "children": [] }
                                    ],
                                    "emoji": ""
                                }
                            ],
                            "emoji": ""
                        }
                    ]
                },
                "trashTree": { "id": "Trash", "children": [] }
            })))
            .mount(&server)
            .await;

        let client = BacklogClient::new(&server.uri(), "KEY").unwrap();
        let tree = client.documents().tree("TEST1").await.unwrap();
        assert_eq!(
            tree.active_tree.children[0].name.as_deref(),
            Some("local test")
        );
    }

    #[tokio::test]
    async fn get_fetches_single_document() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/api/v2/documents/doc-abc123"))
            .respond_with(ResponseTemplate::new(200).set_body_json(document_json()))
            .mount(&server)
            .await;

        let client = BacklogClient::new(&server.uri(), "KEY").unwrap();
        let doc = client.documents().get("doc-abc123").await.unwrap();
        assert_eq!(doc.plain.as_deref(), Some("本文のプレーンテキスト"));
    }

    #[tokio::test]
    async fn get_encodes_document_id_in_path() {
        let server = MockServer::start().await;
        // 悪意ある/特殊文字を含むIDがパスを書き換えないこと
        Mock::given(method("GET"))
            .and(path("/api/v2/documents/..%2Fprojects"))
            .respond_with(ResponseTemplate::new(200).set_body_json(document_json()))
            .expect(1)
            .mount(&server)
            .await;
        // /api/v2/projects に飛んでしまったら失敗するようトラップを張る
        Mock::given(method("GET"))
            .and(path("/api/v2/projects"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!([])))
            .expect(0)
            .mount(&server)
            .await;

        let client = BacklogClient::new(&server.uri(), "KEY").unwrap();
        let doc = client.documents().get("../projects").await.unwrap();
        assert_eq!(doc.id, "doc-abc123");
    }

    #[tokio::test]
    async fn add_posts_form_and_returns_document() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/api/v2/documents"))
            .and(body_string_contains("projectId=7"))
            .respond_with(ResponseTemplate::new(201).set_body_json(document_json()))
            .expect(1)
            .mount(&server)
            .await;

        let client = BacklogClient::new(&server.uri(), "KEY").unwrap();
        let doc = client
            .documents()
            .add(
                AddDocumentParams::new(7)
                    .title("設計メモ")
                    .content("# 見出し"),
            )
            .await
            .unwrap();
        assert_eq!(doc.id, "doc-abc123");
    }

    #[tokio::test]
    async fn comments_fetches_document_comments() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/api/v2/documents/doc-abc123/comments"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!([{
                "id": "cmt-1", "documentId": "doc-abc123",
                "content": "確認しました", "plain": "確認しました",
                "created": "2026-07-21T01:00:00Z", "updated": null,
                "createdUser": { "id": 2, "userId": "eguchi", "name": "eguchi", "roleType": 2 }
            }])))
            .mount(&server)
            .await;

        let client = BacklogClient::new(&server.uri(), "KEY").unwrap();
        let comments = client.documents().comments("doc-abc123").await.unwrap();
        assert_eq!(comments[0].plain.as_deref(), Some("確認しました"));
    }

    #[tokio::test]
    async fn start_attachment_download_fetches_document_file() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/api/v2/documents/doc-abc123/attachments/att-1"))
            .respond_with(
                ResponseTemplate::new(200)
                    .set_body_bytes(vec![4, 5, 6])
                    .insert_header("content-type", "text/plain"),
            )
            .expect(1)
            .mount(&server)
            .await;

        let client = BacklogClient::new(&server.uri(), "KEY").unwrap();
        let download = client
            .documents()
            .start_attachment_download("doc-abc123", "att-1")
            .await
            .unwrap();
        assert_eq!(download.content_type.as_deref(), Some("text/plain"));
        assert_eq!(download.content_length, Some(3));
    }

    #[tokio::test]
    async fn start_attachment_download_encodes_ids_in_path() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/api/v2/documents/..%2Fprojects/attachments/..%2Fx"))
            .respond_with(ResponseTemplate::new(200).set_body_bytes(vec![9, 9, 9]))
            .expect(1)
            .mount(&server)
            .await;

        let client = BacklogClient::new(&server.uri(), "KEY").unwrap();
        let download = client
            .documents()
            .start_attachment_download("../projects", "../x")
            .await
            .unwrap();
        assert_eq!(download.content_length, Some(3));
    }
}
