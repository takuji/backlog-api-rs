use serde::Deserialize;

use crate::models::{GitRepository, PullRequest, PullRequestComment};
use crate::params::QueryPairs;
use crate::{BacklogClient, Result};

use super::{GetCommentsParams, Order};

/// Git / プルリクエスト関連API。`client.git()` で取得する。
#[derive(Debug, Clone, Copy)]
pub struct GitApi<'a> {
    client: &'a BacklogClient,
}

#[derive(Debug, Deserialize)]
struct CountResponse {
    count: u64,
}

/// プルリクエスト一覧・件数のパラメータ。
///
/// <https://developer.nulab.com/docs/backlog/api/2/get-pull-request-list/>
///
/// 並び順の指定はAPIに存在しない。
#[derive(Debug, Clone, Default)]
pub struct GetPullRequestsParams {
    pairs: QueryPairs,
}

impl GetPullRequestsParams {
    pub fn new() -> Self {
        Self::default()
    }

    /// 状態で絞り込む(1=Open, 2=Closed, 3=Merged)。
    pub fn status_ids(mut self, ids: impl IntoIterator<Item = u64>) -> Self {
        self.pairs.push_array("statusId[]", ids);
        self
    }

    /// 担当者で絞り込む。
    pub fn assignee_ids(mut self, ids: impl IntoIterator<Item = u64>) -> Self {
        self.pairs.push_array("assigneeId[]", ids);
        self
    }

    /// 関連課題で絞り込む。
    pub fn issue_ids(mut self, ids: impl IntoIterator<Item = u64>) -> Self {
        self.pairs.push_array("issueId[]", ids);
        self
    }

    /// 作成者で絞り込む。
    pub fn created_user_ids(mut self, ids: impl IntoIterator<Item = u64>) -> Self {
        self.pairs.push_array("createdUserId[]", ids);
        self
    }

    /// 取得件数(1-100、APIの既定は20)。
    pub fn count(mut self, count: u64) -> Self {
        self.pairs.push("count", count);
        self
    }

    /// 取得開始位置。
    pub fn offset(mut self, offset: u64) -> Self {
        self.pairs.push("offset", offset);
        self
    }

    /// 並び替えの対象。
    ///
    /// サーバー実装(`PullRequestQueryForm` / `PullRequestSort`)が受け付けるのは
    /// `"number"` / `"issue"` / `"status"` / `"assigner"` / `"createdUser"` /
    /// `"created"` / `"updated"` の7つで、既定は `"number"`。
    /// **担当者は `assignee` ではなく `assigner` である**(サーバー実装の綴り)。
    ///
    /// 公式APIリファレンスはこのパラメータを記載していないが、サーバーの
    /// フォーム定義が受け付けている。
    pub fn sort(mut self, key: &str) -> Self {
        self.pairs.push("sort", key);
        self
    }

    /// 並び順。**APIの既定は昇順**(古い順)。
    ///
    /// 公式APIリファレンスはこのパラメータを記載していないが、サーバー実装の
    /// `PullRequestQueryForm` が `order` を受け付け、既定を `Order.Ascending` に
    /// している。件数上限が100なので、既定のままだと**最も古い100件**が返る。
    pub fn order(mut self, order: Order) -> Self {
        self.pairs.push("order", order.as_str());
        self
    }

    pub(crate) fn into_pairs(self) -> QueryPairs {
        self.pairs
    }
}

impl<'a> GitApi<'a> {
    pub(crate) fn new(client: &'a BacklogClient) -> Self {
        Self { client }
    }

    /// プロジェクトのGitリポジトリ一覧。
    pub async fn repositories(&self, project_id_or_key: &str) -> Result<Vec<GitRepository>> {
        self.client
            .get(
                &crate::client::api_path(&["projects", project_id_or_key, "git", "repositories"]),
                QueryPairs::new(),
            )
            .await
    }

    /// プルリクエスト一覧。
    pub async fn pull_requests(
        &self,
        project_id_or_key: &str,
        repo_id_or_name: &str,
        params: GetPullRequestsParams,
    ) -> Result<Vec<PullRequest>> {
        self.client
            .get(
                &crate::client::api_path(&[
                    "projects",
                    project_id_or_key,
                    "git",
                    "repositories",
                    repo_id_or_name,
                    "pullRequests",
                ]),
                params.into_pairs(),
            )
            .await
    }

    /// プルリクエストの件数。
    pub async fn pull_requests_count(
        &self,
        project_id_or_key: &str,
        repo_id_or_name: &str,
        params: GetPullRequestsParams,
    ) -> Result<u64> {
        let resp: CountResponse = self
            .client
            .get(
                &crate::client::api_path(&[
                    "projects",
                    project_id_or_key,
                    "git",
                    "repositories",
                    repo_id_or_name,
                    "pullRequests",
                    "count",
                ]),
                params.into_pairs(),
            )
            .await?;
        Ok(resp.count)
    }

    /// プルリクエスト1件。
    pub async fn pull_request(
        &self,
        project_id_or_key: &str,
        repo_id_or_name: &str,
        number: u64,
    ) -> Result<PullRequest> {
        self.client
            .get(
                &crate::client::api_path(&[
                    "projects",
                    project_id_or_key,
                    "git",
                    "repositories",
                    repo_id_or_name,
                    "pullRequests",
                    &number.to_string(),
                ]),
                QueryPairs::new(),
            )
            .await
    }

    /// プルリクエストのコメント一覧。
    pub async fn pull_request_comments(
        &self,
        project_id_or_key: &str,
        repo_id_or_name: &str,
        number: u64,
        params: GetCommentsParams,
    ) -> Result<Vec<PullRequestComment>> {
        self.client
            .get(
                &crate::client::api_path(&[
                    "projects",
                    project_id_or_key,
                    "git",
                    "repositories",
                    repo_id_or_name,
                    "pullRequests",
                    &number.to_string(),
                    "comments",
                ]),
                params.into_pairs(),
            )
            .await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use wiremock::matchers::{method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    /// 実際に届いたリクエストのクエリを取り出す。
    ///
    /// wiremock の `query_param` は「そのパラメータが在ること」しか検証せず、
    /// **余分なパラメータを拒否しない**。「これだけを送る」ことを固定するには
    /// 届いたクエリ集合そのものを比較する必要がある。
    async fn captured_query_pairs(server: &MockServer) -> Vec<(String, String)> {
        let requests = server.received_requests().await.unwrap();
        let request = requests.first().expect("リクエストが1件も届いていない");

        // APIキーはヘッダ `Backlog-API-Key` で「ちょうど1個」送られ、クエリには
        // 一切載らない。ここを素通しにすると、APIキーがクエリ側に混ざる退行や
        // ヘッダごと落ちる退行を、このヘルパーを使う全テストが見逃してしまう
        // (このプロジェクトでは apiKey 漏洩を過去に3回修正している)。
        let api_key_headers = request.headers.get_all("backlog-api-key").iter().count();
        assert_eq!(
            api_key_headers, 1,
            "APIキーは Backlog-API-Key ヘッダでちょうど1個だけ送られるべき"
        );
        assert_eq!(
            request
                .url
                .query_pairs()
                .filter(|(k, _)| k == "apiKey")
                .count(),
            0,
            "APIキーがクエリパラメータに載っている: {:?}",
            request.url.query()
        );

        request
            .url
            .query_pairs()
            .map(|(k, v)| (k.to_string(), v.to_string()))
            .collect()
    }

    /// クエリの一致を「並び順」ではなく「集合(重複込み)」として比較するためにソートする。
    ///
    /// `HashSet` にはしない: 同じペアが1個のはずが2個届いたという多重度だけの
    /// 違いは集合にすると潰れて検出できない。キー名の取り違えは重複を生むため、
    /// これを見逃すと意味がない。
    fn sorted_pairs(mut pairs: Vec<(String, String)>) -> Vec<(String, String)> {
        pairs.sort();
        pairs
    }

    #[tokio::test]
    async fn repositories_requests_the_project_path() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/api/v2/projects/PROJ/git/repositories"))
            .respond_with(ResponseTemplate::new(200).set_body_raw(
                r#"[{ "id": 1, "projectId": 2, "name": "example-repo" }]"#,
                "application/json",
            ))
            .expect(1)
            .mount(&server)
            .await;

        let client = BacklogClient::new(&server.uri(), "KEY").unwrap();
        let list = client.git().repositories("PROJ").await.unwrap();
        assert_eq!(list.len(), 1);
        assert_eq!(list[0].name, "example-repo");
    }

    /// 6つのビルダーメソッドすべてが正しいクエリ名に対応すること。
    ///
    /// 値をすべて異なる数にしてあるので、キー名の取り違え(`statusId[]` と
    /// `assigneeId[]` の入れ替えなど)や、いずれかの取りこぼしを検出できる。
    /// クエリは完全一致で比較する。
    #[tokio::test]
    async fn pull_requests_threads_all_params() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path(
                "/api/v2/projects/PROJ/git/repositories/example-repo/pullRequests",
            ))
            .respond_with(ResponseTemplate::new(200).set_body_raw("[]", "application/json"))
            .expect(1)
            .mount(&server)
            .await;

        let client = BacklogClient::new(&server.uri(), "KEY").unwrap();
        client
            .git()
            .pull_requests(
                "PROJ",
                "example-repo",
                GetPullRequestsParams::new()
                    .status_ids([1, 2])
                    .assignee_ids([30])
                    .issue_ids([40])
                    .created_user_ids([50])
                    .count(60)
                    .offset(70)
                    .sort("updated")
                    .order(Order::Desc),
            )
            .await
            .unwrap();

        assert_eq!(
            sorted_pairs(captured_query_pairs(&server).await),
            sorted_pairs(vec![
                ("statusId[]".to_string(), "1".to_string()),
                ("statusId[]".to_string(), "2".to_string()),
                ("assigneeId[]".to_string(), "30".to_string()),
                ("issueId[]".to_string(), "40".to_string()),
                ("createdUserId[]".to_string(), "50".to_string()),
                ("count".to_string(), "60".to_string()),
                ("offset".to_string(), "70".to_string()),
                ("sort".to_string(), "updated".to_string()),
                ("order".to_string(), "desc".to_string()),
            ])
        );
    }

    #[tokio::test]
    async fn pull_requests_count_returns_the_count() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path(
                "/api/v2/projects/PROJ/git/repositories/example-repo/pullRequests/count",
            ))
            .respond_with(
                ResponseTemplate::new(200).set_body_raw(r#"{"count": 42}"#, "application/json"),
            )
            .expect(1)
            .mount(&server)
            .await;

        let client = BacklogClient::new(&server.uri(), "KEY").unwrap();
        let n = client
            .git()
            .pull_requests_count(
                "PROJ",
                "example-repo",
                GetPullRequestsParams::new().status_ids([1]),
            )
            .await
            .unwrap();
        assert_eq!(n, 42);
        assert_eq!(
            captured_query_pairs(&server).await,
            vec![("statusId[]".to_string(), "1".to_string())]
        );
    }

    #[tokio::test]
    async fn pull_request_requests_the_number_path() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path(
                "/api/v2/projects/PROJ/git/repositories/example-repo/pullRequests/7",
            ))
            .respond_with(ResponseTemplate::new(200).set_body_raw(
                r#"{ "id": 10, "projectId": 2, "repositoryId": 3, "number": 7 }"#,
                "application/json",
            ))
            .expect(1)
            .mount(&server)
            .await;

        let client = BacklogClient::new(&server.uri(), "KEY").unwrap();
        let pr = client
            .git()
            .pull_request("PROJ", "example-repo", 7)
            .await
            .unwrap();
        assert_eq!(pr.number, 7);
    }

    #[tokio::test]
    async fn pull_request_comments_requests_the_comments_path() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path(
                "/api/v2/projects/PROJ/git/repositories/example-repo/pullRequests/7/comments",
            ))
            .respond_with(ResponseTemplate::new(200).set_body_raw(
                r#"[{ "id": 1, "content": "コメント" }]"#,
                "application/json",
            ))
            .expect(1)
            .mount(&server)
            .await;

        let client = BacklogClient::new(&server.uri(), "KEY").unwrap();
        let list = client
            .git()
            .pull_request_comments(
                "PROJ",
                "example-repo",
                7,
                GetCommentsParams::new().count(50),
            )
            .await
            .unwrap();
        assert_eq!(list.len(), 1);
        assert_eq!(
            captured_query_pairs(&server).await,
            vec![("count".to_string(), "50".to_string())]
        );
    }

    /// プロジェクトキーとリポジトリ名が percent-encode されること。
    ///
    /// どちらも呼び出し側から来る文字列で、リポジトリ名にはドットが入り得る
    /// (`example.git` のような名前)。`api_path` を通さず `format!` で
    /// 組み立てると、`../` を含む入力で別のエンドポイントを叩けてしまう。
    /// 既存のパストラバーサル回帰テストと同じ性質のもの。
    ///
    /// **5メソッドすべてを検証する。** レビューで `api_path` を `format!` に
    /// 差し替えて確認したところ、`pull_requests` 以外の4メソッド
    /// (`repositories` / `pull_requests_count` / `pull_request` /
    /// `pull_request_comments`)はガードが無いまま144件のテストが全部通って
    /// いた。両方の呼び出し側入力(`project_id_or_key` と `repo_id_or_name`)が
    /// 届く経路なので、それぞれのメソッドで両方の位置にトラバーサル入力を
    /// 与えて確認する。
    #[tokio::test]
    async fn path_segments_are_percent_encoded() {
        let server = MockServer::start().await;
        // ドットのみのセグメントは二重エンコードされるため、
        // `../` を含む入力でも `/api/v2/projects/` の下から出られない。
        Mock::given(method("GET"))
            .respond_with(ResponseTemplate::new(200).set_body_raw("[]", "application/json"))
            .mount(&server)
            .await;

        let client = BacklogClient::new(&server.uri(), "KEY").unwrap();

        // project_id_or_key の位置にトラバーサル入力を与える。
        let _ = client.git().repositories("../../users").await;
        let _ = client
            .git()
            .pull_requests("../../users", "..", GetPullRequestsParams::new())
            .await;
        let _ = client
            .git()
            .pull_requests_count("../../users", "..", GetPullRequestsParams::new())
            .await;
        let _ = client.git().pull_request("../../users", "..", 7).await;
        let _ = client
            .git()
            .pull_request_comments("../../users", "..", 7, GetCommentsParams::new())
            .await;

        // repo_id_or_name の位置にトラバーサル入力を与える(project側は正常値)。
        let _ = client
            .git()
            .pull_requests("PROJ", "../../users", GetPullRequestsParams::new())
            .await;
        let _ = client
            .git()
            .pull_requests_count("PROJ", "../../users", GetPullRequestsParams::new())
            .await;
        let _ = client.git().pull_request("PROJ", "../../users", 7).await;
        let _ = client
            .git()
            .pull_request_comments("PROJ", "../../users", 7, GetCommentsParams::new())
            .await;

        let requests = server.received_requests().await.unwrap();
        assert_eq!(requests.len(), 9, "9回のリクエストが届いているはず");
        for request in &requests {
            let p = request.url.path();
            assert!(
                p.starts_with("/api/v2/projects/"),
                "パスが projects の外へ出ている: {p}"
            );
            assert!(
                !p.contains("/users/"),
                "別のエンドポイントを叩いている: {p}"
            );
        }
    }
}
