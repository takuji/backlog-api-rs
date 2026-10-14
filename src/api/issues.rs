use chrono::NaiveDate;
use serde::Deserialize;

use crate::models::{Attachment, Comment, Issue};
use crate::params::QueryPairs;
use crate::{AttachmentDownload, BacklogClient, Result};

/// 課題関連API。`client.issues()` で取得する。
#[derive(Debug, Clone, Copy)]
pub struct IssuesApi<'a> {
    client: &'a BacklogClient,
}

/// ソート順。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Order {
    Asc,
    Desc,
}

impl Order {
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Order::Asc => "asc",
            Order::Desc => "desc",
        }
    }
}

/// 課題一覧の取得(GET /api/v2/issues)・件数取得のパラメータ。
///
/// <https://developer.nulab.com/docs/backlog/api/2/get-issue-list/>
#[derive(Debug, Clone, Default)]
pub struct GetIssuesParams {
    pairs: QueryPairs,
}

impl GetIssuesParams {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn project_ids(mut self, ids: impl IntoIterator<Item = u64>) -> Self {
        self.pairs.push_array("projectId[]", ids);
        self
    }

    pub fn issue_type_ids(mut self, ids: impl IntoIterator<Item = u64>) -> Self {
        self.pairs.push_array("issueTypeId[]", ids);
        self
    }

    pub fn category_ids(mut self, ids: impl IntoIterator<Item = u64>) -> Self {
        self.pairs.push_array("categoryId[]", ids);
        self
    }

    pub fn version_ids(mut self, ids: impl IntoIterator<Item = u64>) -> Self {
        self.pairs.push_array("versionId[]", ids);
        self
    }

    pub fn milestone_ids(mut self, ids: impl IntoIterator<Item = u64>) -> Self {
        self.pairs.push_array("milestoneId[]", ids);
        self
    }

    pub fn status_ids(mut self, ids: impl IntoIterator<Item = u64>) -> Self {
        self.pairs.push_array("statusId[]", ids);
        self
    }

    pub fn not_status_ids(mut self, ids: impl IntoIterator<Item = u64>) -> Self {
        self.pairs.push_array("notStatusId[]", ids);
        self
    }

    pub fn priority_ids(mut self, ids: impl IntoIterator<Item = u64>) -> Self {
        self.pairs.push_array("priorityId[]", ids);
        self
    }

    pub fn assignee_ids(mut self, ids: impl IntoIterator<Item = u64>) -> Self {
        self.pairs.push_array("assigneeId[]", ids);
        self
    }

    pub fn created_user_ids(mut self, ids: impl IntoIterator<Item = u64>) -> Self {
        self.pairs.push_array("createdUserId[]", ids);
        self
    }

    pub fn resolution_ids(mut self, ids: impl IntoIterator<Item = u64>) -> Self {
        self.pairs.push_array("resolutionId[]", ids);
        self
    }

    pub fn ids(mut self, ids: impl IntoIterator<Item = u64>) -> Self {
        self.pairs.push_array("id[]", ids);
        self
    }

    pub fn parent_issue_ids(mut self, ids: impl IntoIterator<Item = u64>) -> Self {
        self.pairs.push_array("parentIssueId[]", ids);
        self
    }

    /// 親子課題の絞り込み(0=すべて, 1=子課題以外, 2=子課題, 3=親も子もない課題, 4=親課題)。
    pub fn parent_child(mut self, value: u8) -> Self {
        self.pairs.push("parentChild", value);
        self
    }

    pub fn attachment(mut self, value: bool) -> Self {
        self.pairs.push("attachment", value);
        self
    }

    pub fn shared_file(mut self, value: bool) -> Self {
        self.pairs.push("sharedFile", value);
        self
    }

    /// ソート対象の属性名(`"created"`, `"updated"`, `"priority"` など)。
    pub fn sort(mut self, key: &str) -> Self {
        self.pairs.push("sort", key);
        self
    }

    pub fn order(mut self, order: Order) -> Self {
        self.pairs.push("order", order.as_str());
        self
    }

    pub fn keyword(mut self, keyword: &str) -> Self {
        self.pairs.push("keyword", keyword);
        self
    }

    pub fn offset(mut self, offset: u64) -> Self {
        self.pairs.push("offset", offset);
        self
    }

    /// 取得上限(1-100、デフォルト20)。
    pub fn count(mut self, count: u64) -> Self {
        self.pairs.push("count", count);
        self
    }

    pub fn created_since(mut self, date: NaiveDate) -> Self {
        self.pairs.push("createdSince", date.format("%Y-%m-%d"));
        self
    }

    pub fn created_until(mut self, date: NaiveDate) -> Self {
        self.pairs.push("createdUntil", date.format("%Y-%m-%d"));
        self
    }

    pub fn updated_since(mut self, date: NaiveDate) -> Self {
        self.pairs.push("updatedSince", date.format("%Y-%m-%d"));
        self
    }

    pub fn updated_until(mut self, date: NaiveDate) -> Self {
        self.pairs.push("updatedUntil", date.format("%Y-%m-%d"));
        self
    }

    pub fn start_date_since(mut self, date: NaiveDate) -> Self {
        self.pairs.push("startDateSince", date.format("%Y-%m-%d"));
        self
    }

    pub fn start_date_until(mut self, date: NaiveDate) -> Self {
        self.pairs.push("startDateUntil", date.format("%Y-%m-%d"));
        self
    }

    pub fn due_date_since(mut self, date: NaiveDate) -> Self {
        self.pairs.push("dueDateSince", date.format("%Y-%m-%d"));
        self
    }

    pub fn due_date_until(mut self, date: NaiveDate) -> Self {
        self.pairs.push("dueDateUntil", date.format("%Y-%m-%d"));
        self
    }

    pub(crate) fn into_pairs(self) -> QueryPairs {
        self.pairs
    }
}

/// 課題の追加(POST /api/v2/issues)のパラメータ。
///
/// <https://developer.nulab.com/docs/backlog/api/2/add-issue/>
#[derive(Debug, Clone)]
pub struct AddIssueParams {
    pairs: QueryPairs,
}

impl AddIssueParams {
    /// 必須パラメータを指定して作る。`priority_id` は 2=高, 3=中, 4=低。
    pub fn new(project_id: u64, summary: &str, issue_type_id: u64, priority_id: u64) -> Self {
        let mut pairs = QueryPairs::new();
        pairs.push("projectId", project_id);
        pairs.push("summary", summary);
        pairs.push("issueTypeId", issue_type_id);
        pairs.push("priorityId", priority_id);
        Self { pairs }
    }

    pub fn description(mut self, description: &str) -> Self {
        self.pairs.push("description", description);
        self
    }

    pub fn parent_issue_id(mut self, id: u64) -> Self {
        self.pairs.push("parentIssueId", id);
        self
    }

    pub fn start_date(mut self, date: NaiveDate) -> Self {
        self.pairs.push("startDate", date.format("%Y-%m-%d"));
        self
    }

    pub fn due_date(mut self, date: NaiveDate) -> Self {
        self.pairs.push("dueDate", date.format("%Y-%m-%d"));
        self
    }

    pub fn estimated_hours(mut self, hours: f64) -> Self {
        self.pairs.push("estimatedHours", hours);
        self
    }

    pub fn actual_hours(mut self, hours: f64) -> Self {
        self.pairs.push("actualHours", hours);
        self
    }

    pub fn category_ids(mut self, ids: impl IntoIterator<Item = u64>) -> Self {
        self.pairs.push_array("categoryId[]", ids);
        self
    }

    pub fn version_ids(mut self, ids: impl IntoIterator<Item = u64>) -> Self {
        self.pairs.push_array("versionId[]", ids);
        self
    }

    pub fn milestone_ids(mut self, ids: impl IntoIterator<Item = u64>) -> Self {
        self.pairs.push_array("milestoneId[]", ids);
        self
    }

    pub fn assignee_id(mut self, id: u64) -> Self {
        self.pairs.push("assigneeId", id);
        self
    }

    pub fn notified_user_ids(mut self, ids: impl IntoIterator<Item = u64>) -> Self {
        self.pairs.push_array("notifiedUserId[]", ids);
        self
    }

    pub fn attachment_ids(mut self, ids: impl IntoIterator<Item = u64>) -> Self {
        self.pairs.push_array("attachmentId[]", ids);
        self
    }

    pub(crate) fn into_pairs(self) -> QueryPairs {
        self.pairs
    }
}

/// 課題の更新(PATCH /api/v2/issues/{issueIdOrKey})のパラメータ。すべて任意。
///
/// <https://developer.nulab.com/docs/backlog/api/2/update-issue/>
#[derive(Debug, Clone, Default)]
pub struct UpdateIssueParams {
    pairs: QueryPairs,
}

impl UpdateIssueParams {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn summary(mut self, summary: &str) -> Self {
        self.pairs.push("summary", summary);
        self
    }

    pub fn description(mut self, description: &str) -> Self {
        self.pairs.push("description", description);
        self
    }

    pub fn status_id(mut self, id: u64) -> Self {
        self.pairs.push("statusId", id);
        self
    }

    pub fn resolution_id(mut self, id: u64) -> Self {
        self.pairs.push("resolutionId", id);
        self
    }

    pub fn issue_type_id(mut self, id: u64) -> Self {
        self.pairs.push("issueTypeId", id);
        self
    }

    pub fn priority_id(mut self, id: u64) -> Self {
        self.pairs.push("priorityId", id);
        self
    }

    pub fn assignee_id(mut self, id: u64) -> Self {
        self.pairs.push("assigneeId", id);
        self
    }

    pub fn parent_issue_id(mut self, id: u64) -> Self {
        self.pairs.push("parentIssueId", id);
        self
    }

    pub fn start_date(mut self, date: NaiveDate) -> Self {
        self.pairs.push("startDate", date.format("%Y-%m-%d"));
        self
    }

    pub fn due_date(mut self, date: NaiveDate) -> Self {
        self.pairs.push("dueDate", date.format("%Y-%m-%d"));
        self
    }

    pub fn estimated_hours(mut self, hours: f64) -> Self {
        self.pairs.push("estimatedHours", hours);
        self
    }

    pub fn actual_hours(mut self, hours: f64) -> Self {
        self.pairs.push("actualHours", hours);
        self
    }

    pub fn category_ids(mut self, ids: impl IntoIterator<Item = u64>) -> Self {
        self.pairs.push_array("categoryId[]", ids);
        self
    }

    pub fn version_ids(mut self, ids: impl IntoIterator<Item = u64>) -> Self {
        self.pairs.push_array("versionId[]", ids);
        self
    }

    pub fn milestone_ids(mut self, ids: impl IntoIterator<Item = u64>) -> Self {
        self.pairs.push_array("milestoneId[]", ids);
        self
    }

    pub fn notified_user_ids(mut self, ids: impl IntoIterator<Item = u64>) -> Self {
        self.pairs.push_array("notifiedUserId[]", ids);
        self
    }

    pub fn attachment_ids(mut self, ids: impl IntoIterator<Item = u64>) -> Self {
        self.pairs.push_array("attachmentId[]", ids);
        self
    }

    /// 更新と同時に投稿するコメント。
    pub fn comment(mut self, comment: &str) -> Self {
        self.pairs.push("comment", comment);
        self
    }

    pub(crate) fn into_pairs(self) -> QueryPairs {
        self.pairs
    }
}

/// コメント一覧の取得(GET /api/v2/issues/{issueIdOrKey}/comments)のパラメータ。
///
/// <https://developer.nulab.com/docs/backlog/api/2/get-comment-list/>
#[derive(Debug, Clone, Default)]
pub struct GetCommentsParams {
    pairs: QueryPairs,
}

impl GetCommentsParams {
    pub fn new() -> Self {
        Self::default()
    }

    /// このID以降のコメントを取得する。
    pub fn min_id(mut self, id: u64) -> Self {
        self.pairs.push("minId", id);
        self
    }

    /// このID以前のコメントを取得する。
    pub fn max_id(mut self, id: u64) -> Self {
        self.pairs.push("maxId", id);
        self
    }

    /// 取得上限(1-100、デフォルト20)。
    pub fn count(mut self, count: u64) -> Self {
        self.pairs.push("count", count);
        self
    }

    pub fn order(mut self, order: Order) -> Self {
        self.pairs.push("order", order.as_str());
        self
    }

    pub(crate) fn into_pairs(self) -> QueryPairs {
        self.pairs
    }
}

/// コメントの追加(POST /api/v2/issues/{issueIdOrKey}/comments)のパラメータ。
///
/// <https://developer.nulab.com/docs/backlog/api/2/add-comment/>
#[derive(Debug, Clone)]
pub struct AddCommentParams {
    pairs: QueryPairs,
}

impl AddCommentParams {
    pub fn new(content: &str) -> Self {
        let mut pairs = QueryPairs::new();
        pairs.push("content", content);
        Self { pairs }
    }

    pub fn notified_user_ids(mut self, ids: impl IntoIterator<Item = u64>) -> Self {
        self.pairs.push_array("notifiedUserId[]", ids);
        self
    }

    pub fn attachment_ids(mut self, ids: impl IntoIterator<Item = u64>) -> Self {
        self.pairs.push_array("attachmentId[]", ids);
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

impl<'a> IssuesApi<'a> {
    pub(crate) fn new(client: &'a BacklogClient) -> Self {
        Self { client }
    }

    /// 課題一覧を取得する。
    pub async fn list(&self, params: GetIssuesParams) -> Result<Vec<Issue>> {
        self.client.get("/api/v2/issues", params.into_pairs()).await
    }

    /// 条件に合う課題数を取得する。
    pub async fn count(&self, params: GetIssuesParams) -> Result<u64> {
        let resp: CountResponse = self
            .client
            .get("/api/v2/issues/count", params.into_pairs())
            .await?;
        Ok(resp.count)
    }

    /// 課題を取得する。`issue_id_or_key` はID数値または課題キー(`"PROJ-123"`)。
    pub async fn get(&self, issue_id_or_key: &str) -> Result<Issue> {
        self.client
            .get(
                &crate::client::api_path(&["issues", issue_id_or_key]),
                QueryPairs::new(),
            )
            .await
    }

    /// 課題を追加する。
    pub async fn add(&self, params: AddIssueParams) -> Result<Issue> {
        self.client
            .post("/api/v2/issues", params.into_pairs())
            .await
    }

    /// 課題を更新する。
    pub async fn update(&self, issue_id_or_key: &str, params: UpdateIssueParams) -> Result<Issue> {
        self.client
            .patch(
                &crate::client::api_path(&["issues", issue_id_or_key]),
                params.into_pairs(),
            )
            .await
    }

    /// 課題を削除する。削除された課題を返す。
    pub async fn delete(&self, issue_id_or_key: &str) -> Result<Issue> {
        self.client
            .delete(&crate::client::api_path(&["issues", issue_id_or_key]))
            .await
    }

    /// 課題のコメント一覧を取得する。
    pub async fn comments(
        &self,
        issue_id_or_key: &str,
        params: GetCommentsParams,
    ) -> Result<Vec<Comment>> {
        self.client
            .get(
                &crate::client::api_path(&["issues", issue_id_or_key, "comments"]),
                params.into_pairs(),
            )
            .await
    }

    /// 課題にコメントを追加する。
    pub async fn add_comment(
        &self,
        issue_id_or_key: &str,
        params: AddCommentParams,
    ) -> Result<Comment> {
        self.client
            .post(
                &crate::client::api_path(&["issues", issue_id_or_key, "comments"]),
                params.into_pairs(),
            )
            .await
    }

    /// 課題の添付ファイル一覧を取得する。
    pub async fn attachments(&self, issue_id_or_key: &str) -> Result<Vec<Attachment>> {
        self.client
            .get(
                &crate::client::api_path(&["issues", issue_id_or_key, "attachments"]),
                QueryPairs::new(),
            )
            .await
    }

    /// 課題の添付ファイルのダウンロードを開始する。
    ///
    /// 返ったハンドルの [`AttachmentDownload::save_to`] でファイルに書き出す。
    pub async fn start_attachment_download(
        &self,
        issue_id_or_key: &str,
        attachment_id: u64,
    ) -> Result<AttachmentDownload> {
        self.client
            .start_download(&crate::client::api_path(&[
                "issues",
                issue_id_or_key,
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

    fn issue_json() -> serde_json::Value {
        json!({
            "id": 1, "projectId": 1, "issueKey": "BLG-1", "keyId": 1,
            "issueType": { "id": 2, "projectId": 1, "name": "タスク", "color": "#7ea800", "displayOrder": 0 },
            "summary": "first issue", "description": "",
            "resolution": null, "priority": { "id": 3, "name": "中" },
            "status": { "id": 1, "projectId": 1, "name": "未対応", "color": "#ed8077", "displayOrder": 1000 },
            "assignee": null, "category": [], "versions": [], "milestone": [],
            "startDate": null, "dueDate": null,
            "estimatedHours": null, "actualHours": null, "parentIssueId": null,
            "createdUser": { "id": 1, "userId": "admin", "name": "admin", "roleType": 1 },
            "created": "2012-07-23T06:10:15Z",
            "updatedUser": null, "updated": null,
            "customFields": [], "attachments": [], "sharedFiles": [], "stars": []
        })
    }

    #[test]
    fn get_issues_params_builds_query_pairs() {
        let params = GetIssuesParams::new()
            .project_ids([1u64, 2])
            .status_ids([1u64])
            .keyword("bug")
            .sort("updated")
            .order(Order::Asc)
            .offset(10)
            .count(50);
        let pairs = params.into_pairs();
        let slice = pairs.as_slice();
        assert!(slice.contains(&("projectId[]".to_string(), "1".to_string())));
        assert!(slice.contains(&("projectId[]".to_string(), "2".to_string())));
        assert!(slice.contains(&("statusId[]".to_string(), "1".to_string())));
        assert!(slice.contains(&("keyword".to_string(), "bug".to_string())));
        assert!(slice.contains(&("sort".to_string(), "updated".to_string())));
        assert!(slice.contains(&("order".to_string(), "asc".to_string())));
        assert!(slice.contains(&("offset".to_string(), "10".to_string())));
        assert!(slice.contains(&("count".to_string(), "50".to_string())));
    }

    #[test]
    fn date_params_are_formatted_as_yyyy_mm_dd() {
        let since = chrono::NaiveDate::from_ymd_opt(2024, 4, 1).unwrap();
        let params = GetIssuesParams::new().created_since(since);
        assert!(
            params
                .into_pairs()
                .as_slice()
                .contains(&("createdSince".to_string(), "2024-04-01".to_string()))
        );
    }

    #[tokio::test]
    async fn list_fetches_issues_with_params() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/api/v2/issues"))
            .and(query_param("projectId[]", "1"))
            .and(query_param("count", "2"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!([issue_json()])))
            .expect(1)
            .mount(&server)
            .await;

        let client = BacklogClient::new(&server.uri(), "KEY").unwrap();
        let issues = client
            .issues()
            .list(GetIssuesParams::new().project_ids([1u64]).count(2))
            .await
            .unwrap();
        assert_eq!(issues[0].issue_key, "BLG-1");
    }

    #[tokio::test]
    async fn count_unwraps_count_field() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/api/v2/issues/count"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({ "count": 42 })))
            .mount(&server)
            .await;

        let client = BacklogClient::new(&server.uri(), "KEY").unwrap();
        let count = client.issues().count(GetIssuesParams::new()).await.unwrap();
        assert_eq!(count, 42);
    }

    #[tokio::test]
    async fn get_fetches_single_issue() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/api/v2/issues/BLG-1"))
            .respond_with(ResponseTemplate::new(200).set_body_json(issue_json()))
            .mount(&server)
            .await;

        let client = BacklogClient::new(&server.uri(), "KEY").unwrap();
        let issue = client.issues().get("BLG-1").await.unwrap();
        assert_eq!(issue.id, 1);
    }

    /// `issue_id_or_key` に `?` を含む値を渡しても、クエリパラメータとして
    /// 注入されず、パスセグメントの一部としてpercent-encodeされることを確認する
    /// (レビュー指摘: パスセグメント未エンコード)。
    #[tokio::test]
    async fn get_percent_encodes_query_like_input_in_path_segment() {
        let server = MockServer::start().await;
        // 脆弱だった場合にヒットしてしまう「罠」モック: 素通しなら
        // /api/v2/issues/BLG-1?archived=false というリクエストになり、
        // これにマッチしてしまう。
        Mock::given(method("GET"))
            .and(path("/api/v2/issues/BLG-1"))
            .and(query_param("archived", "false"))
            .respond_with(ResponseTemplate::new(200).set_body_json(issue_json()))
            .expect(0)
            .mount(&server)
            .await;
        // 正しくpercent-encodeされた場合にヒットするモック。
        Mock::given(method("GET"))
            .and(path("/api/v2/issues/BLG-1%3Farchived=false"))
            .respond_with(ResponseTemplate::new(200).set_body_json(issue_json()))
            .expect(1)
            .mount(&server)
            .await;

        let client = BacklogClient::new(&server.uri(), "KEY").unwrap();
        let issue = client.issues().get("BLG-1?archived=false").await.unwrap();
        assert_eq!(issue.id, 1);
    }

    /// `issue_id_or_key` に `..` を含む値を渡しても、別のエンドポイント
    /// (`/api/v2/projects` など)にパストラバーサルできないことを確認する
    /// (レビュー指摘: パスセグメント未エンコード)。
    #[tokio::test]
    async fn get_does_not_allow_path_traversal_via_dot_segments() {
        let server = MockServer::start().await;
        // 脆弱だった場合にヒットしてしまう「罠」モック。
        Mock::given(method("GET"))
            .and(path("/api/v2/projects"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!([project_like_json()])))
            .expect(0)
            .mount(&server)
            .await;

        let client = BacklogClient::new(&server.uri(), "KEY").unwrap();
        let result = client.issues().get("../projects").await;
        // トラバーサルが起きなければ、罠モックにはマッチせず404などのエラーになる。
        assert!(result.is_err(), "expected error, got {result:?}");
    }

    fn project_like_json() -> serde_json::Value {
        json!({ "id": 999 })
    }

    #[test]
    fn add_issue_params_builds_form_pairs() {
        let params = AddIssueParams::new(1, "new issue", 2, 3)
            .description("desc")
            .assignee_id(5)
            .category_ids([12u64])
            .due_date(chrono::NaiveDate::from_ymd_opt(2024, 5, 1).unwrap());
        let pairs = params.into_pairs();
        let slice = pairs.as_slice();
        assert!(slice.contains(&("projectId".to_string(), "1".to_string())));
        assert!(slice.contains(&("summary".to_string(), "new issue".to_string())));
        assert!(slice.contains(&("issueTypeId".to_string(), "2".to_string())));
        assert!(slice.contains(&("priorityId".to_string(), "3".to_string())));
        assert!(slice.contains(&("description".to_string(), "desc".to_string())));
        assert!(slice.contains(&("assigneeId".to_string(), "5".to_string())));
        assert!(slice.contains(&("categoryId[]".to_string(), "12".to_string())));
        assert!(slice.contains(&("dueDate".to_string(), "2024-05-01".to_string())));
    }

    #[tokio::test]
    async fn add_posts_form_and_returns_issue() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/api/v2/issues"))
            .and(body_string_contains("projectId=1"))
            .and(body_string_contains("summary=new+issue"))
            .respond_with(ResponseTemplate::new(201).set_body_json(issue_json()))
            .expect(1)
            .mount(&server)
            .await;

        let client = BacklogClient::new(&server.uri(), "KEY").unwrap();
        let issue = client
            .issues()
            .add(AddIssueParams::new(1, "new issue", 2, 3))
            .await
            .unwrap();
        assert_eq!(issue.id, 1);
    }

    #[tokio::test]
    async fn update_patches_form() {
        let server = MockServer::start().await;
        Mock::given(method("PATCH"))
            .and(path("/api/v2/issues/BLG-1"))
            .and(body_string_contains("statusId=3"))
            .and(body_string_contains("comment=done"))
            .respond_with(ResponseTemplate::new(200).set_body_json(issue_json()))
            .expect(1)
            .mount(&server)
            .await;

        let client = BacklogClient::new(&server.uri(), "KEY").unwrap();
        let issue = client
            .issues()
            .update(
                "BLG-1",
                UpdateIssueParams::new().status_id(3).comment("done"),
            )
            .await
            .unwrap();
        assert_eq!(issue.issue_key, "BLG-1");
    }

    #[tokio::test]
    async fn delete_returns_deleted_issue() {
        let server = MockServer::start().await;
        Mock::given(method("DELETE"))
            .and(path("/api/v2/issues/BLG-1"))
            .respond_with(ResponseTemplate::new(200).set_body_json(issue_json()))
            .expect(1)
            .mount(&server)
            .await;

        let client = BacklogClient::new(&server.uri(), "KEY").unwrap();
        let issue = client.issues().delete("BLG-1").await.unwrap();
        assert_eq!(issue.id, 1);
    }

    fn comment_json() -> serde_json::Value {
        json!({
            "id": 6586, "projectId": 1, "issueId": 1102,
            // 公式ドキュメント(get-comment-list)の応答例は changeLog が null。
            // `#[serde(default)]` だけでは失敗するため、ここを null にしておくことで
            // null_as_empty_vec が効いていることを常に検証する。
            "content": "test", "changeLog": null,
            "createdUser": { "id": 1, "userId": "admin", "name": "admin", "roleType": 1 },
            "created": "2013-08-05T06:15:06Z", "updated": "2013-08-05T06:15:06Z",
            "stars": [], "notifications": []
        })
    }

    #[tokio::test]
    async fn comments_fetches_with_params() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/api/v2/issues/BLG-1/comments"))
            .and(query_param("count", "10"))
            .and(query_param("order", "asc"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!([comment_json()])))
            .expect(1)
            .mount(&server)
            .await;

        let client = BacklogClient::new(&server.uri(), "KEY").unwrap();
        let comments = client
            .issues()
            .comments(
                "BLG-1",
                GetCommentsParams::new().count(10).order(Order::Asc),
            )
            .await
            .unwrap();
        assert_eq!(comments[0].content.as_deref(), Some("test"));
    }

    #[tokio::test]
    async fn add_comment_posts_content() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/api/v2/issues/BLG-1/comments"))
            .and(body_string_contains("content=test"))
            .respond_with(ResponseTemplate::new(201).set_body_json(comment_json()))
            .expect(1)
            .mount(&server)
            .await;

        let client = BacklogClient::new(&server.uri(), "KEY").unwrap();
        let comment = client
            .issues()
            .add_comment("BLG-1", AddCommentParams::new("test"))
            .await
            .unwrap();
        assert_eq!(comment.id, 6586);
    }

    #[tokio::test]
    async fn attachments_lists_issue_attachments() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/api/v2/issues/TEST1-1/attachments"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!([
                { "id": 22067, "name": "spec.pdf", "size": 8718 }
            ])))
            .mount(&server)
            .await;

        let client = BacklogClient::new(&server.uri(), "KEY").unwrap();
        let list = client.issues().attachments("TEST1-1").await.unwrap();
        assert_eq!(list[0].id, 22067);
        assert_eq!(list[0].name, "spec.pdf");
    }

    #[tokio::test]
    async fn start_attachment_download_exposes_headers() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/api/v2/issues/TEST1-1/attachments/22067"))
            .respond_with(
                ResponseTemplate::new(200)
                    .set_body_bytes(vec![1, 2, 3])
                    .insert_header("content-type", "application/pdf"),
            )
            .expect(1)
            .mount(&server)
            .await;

        let client = BacklogClient::new(&server.uri(), "KEY").unwrap();
        let download = client
            .issues()
            .start_attachment_download("TEST1-1", 22067)
            .await
            .unwrap();
        assert_eq!(download.content_type.as_deref(), Some("application/pdf"));
        assert_eq!(download.content_length, Some(3));
    }

    #[tokio::test]
    async fn start_attachment_download_encodes_issue_key_in_path() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/api/v2/issues/..%2Fprojects/attachments/1"))
            // 3バイト = 正しいパスに届いた印
            .respond_with(ResponseTemplate::new(200).set_body_bytes(vec![9, 9, 9]))
            .expect(1)
            .mount(&server)
            .await;
        Mock::given(method("GET"))
            .and(path("/api/v2/projects/attachments/1"))
            .respond_with(ResponseTemplate::new(200).set_body_bytes(vec![0]))
            .expect(0)
            .mount(&server)
            .await;

        let client = BacklogClient::new(&server.uri(), "KEY").unwrap();
        let download = client
            .issues()
            .start_attachment_download("../projects", 1)
            .await
            .unwrap();
        assert_eq!(download.content_length, Some(3));
    }
}
