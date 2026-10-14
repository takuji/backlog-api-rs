use serde::Deserialize;

use crate::models::Watching;
use crate::params::QueryPairs;
use crate::{BacklogClient, Result};

use super::Order;

/// ウォッチ関連API。`client.watchings()` で取得する。
#[derive(Debug, Clone, Copy)]
pub struct WatchingsApi<'a> {
    client: &'a BacklogClient,
}

/// ウォッチ一覧の取得(GET /api/v2/users/{userId}/watchings)のパラメータ。
///
/// <https://developer.nulab.com/docs/backlog/api/2/get-watching-list/>
#[derive(Debug, Clone, Default)]
pub struct GetWatchingsParams {
    pairs: QueryPairs,
}

impl GetWatchingsParams {
    pub fn new() -> Self {
        Self::default()
    }

    /// 取得件数(1-100、APIの既定は20)。
    pub fn count(mut self, count: u64) -> Self {
        self.pairs.push("count", count);
        self
    }

    /// 取得開始位置(APIの既定は0)。
    pub fn offset(mut self, offset: u64) -> Self {
        self.pairs.push("offset", offset);
        self
    }

    /// ソート対象(`"created"` / `"updated"` / `"issueUpdated"`。APIの既定は `issueUpdated`)。
    pub fn sort(mut self, key: &str) -> Self {
        self.pairs.push("sort", key);
        self
    }

    /// 並び順(APIの既定は desc)。
    pub fn order(mut self, order: Order) -> Self {
        self.pairs.push("order", order.as_str());
        self
    }

    /// 既読状態で絞る。`false` を渡すと更新のある(未読の)ウォッチだけになる。
    pub fn resource_already_read(mut self, read: bool) -> Self {
        self.pairs.push("resourceAlreadyRead", read);
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

impl<'a> WatchingsApi<'a> {
    pub(crate) fn new(client: &'a BacklogClient) -> Self {
        Self { client }
    }

    /// ユーザーのウォッチ一覧を取得する。
    ///
    /// APIがユーザーIDを要求するため素直に受ける。「自分の」を解決するのは
    /// 呼び出し側の責務(backlog-mcp が `get_myself` で解決する)。
    pub async fn list_for_user(
        &self,
        user_id: u64,
        params: GetWatchingsParams,
    ) -> Result<Vec<Watching>> {
        self.client
            .get(
                &crate::client::api_path(&["users", &user_id.to_string(), "watchings"]),
                params.into_pairs(),
            )
            .await
    }

    /// 更新のある(未読の)ウォッチ数を取得する(`resourceAlreadyRead=false`)。
    pub async fn unread_count_for_user(&self, user_id: u64) -> Result<u64> {
        let mut query = QueryPairs::new();
        query.push("resourceAlreadyRead", false);
        let resp: CountResponse = self
            .client
            .get(
                &crate::client::api_path(&["users", &user_id.to_string(), "watchings", "count"]),
                query,
            )
            .await?;
        Ok(resp.count)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use wiremock::matchers::{method, path, query_param};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    /// 実際に届いたリクエストのクエリを取り出す。APIキーはヘッダで送るので、
    /// ここに現れるのはAPIリソース側が組み立てたパラメータだけになる。
    ///
    /// `query_param` マッチャーはあるキーが指定した値で「存在すること」しか
    /// 検証できず、余計なパラメータが混じっていても素通りしてしまう。
    /// 完全一致を保証するには、実際に届いたリクエストを捕まえて
    /// クエリ文字列そのものを比較する必要がある。
    async fn captured_query_pairs(server: &MockServer) -> Vec<(String, String)> {
        let requests = server.received_requests().await.unwrap();
        // `requests[0]` だと空のときに "index out of bounds" としか出ず、
        // モックにリクエストが届いていないことが分かりにくいため、
        // 意図の分かるメッセージ付きで落とす。
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

    /// クエリパラメータの一致を「並び順」ではなく「集合(重複込み)」として
    /// 比較するためにソートする。
    ///
    /// `HashSet<(String, String)>` にはしない: `("count", "50")` のような
    /// ペア単位で見れば異なるキー・値の取り違えはペアのまま区別できるが、
    /// 「同じペアが1個だけ届くべきところ2個届いた」という多重度だけの違いは
    /// 集合にした時点で潰れてしまい検出できない。ソート済み `Vec` なら
    /// 重複を保持したまま並び順の違いだけを無視できる。
    fn sorted_pairs(mut pairs: Vec<(String, String)>) -> Vec<(String, String)> {
        pairs.sort();
        pairs
    }

    #[tokio::test]
    async fn list_requests_watchings_for_the_user() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/api/v2/users/1234/watchings"))
            .respond_with(ResponseTemplate::new(200).set_body_raw(
                r#"[{ "id": 1, "resourceAlreadyRead": false,
                      "created": "2013-10-31T06:58:59Z" }]"#,
                "application/json",
            ))
            .expect(1)
            .mount(&server)
            .await;

        let client = BacklogClient::new(&server.uri(), "KEY").unwrap();
        let list = client
            .watchings()
            .list_for_user(
                1234,
                GetWatchingsParams::new()
                    .count(50)
                    .offset(10)
                    .sort("issueUpdated")
                    .order(Order::Asc)
                    .resource_already_read(false),
            )
            .await
            .unwrap();
        assert_eq!(list.len(), 1);
        assert_eq!(list[0].id, 1);

        // 5つのパラメータそれぞれが正しい名前・値で送られていること、かつ
        // それ以外のパラメータが混じっていないことを厳密に確認する。
        // (例えば count と offset を取り違えても query_param マッチャーだけでは
        // 検出できない)
        assert_eq!(
            sorted_pairs(captured_query_pairs(&server).await),
            sorted_pairs(vec![
                ("count".to_string(), "50".to_string()),
                ("offset".to_string(), "10".to_string()),
                ("sort".to_string(), "issueUpdated".to_string()),
                ("order".to_string(), "asc".to_string()),
                ("resourceAlreadyRead".to_string(), "false".to_string()),
            ])
        );
    }

    /// 更新のあるウォッチ数は `resourceAlreadyRead=false` を「だけ」送ること。
    #[tokio::test]
    async fn unread_count_filters_by_resource_already_read_false() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/api/v2/users/1234/watchings/count"))
            .and(query_param("resourceAlreadyRead", "false"))
            .respond_with(
                ResponseTemplate::new(200).set_body_raw(r#"{"count": 3}"#, "application/json"),
            )
            .expect(1)
            .mount(&server)
            .await;

        let client = BacklogClient::new(&server.uri(), "KEY").unwrap();
        assert_eq!(
            client
                .watchings()
                .unread_count_for_user(1234)
                .await
                .unwrap(),
            3
        );

        // resourceAlreadyRead=false 以外のクエリパラメータが一切ないことを確認する。
        assert_eq!(
            sorted_pairs(captured_query_pairs(&server).await),
            sorted_pairs(vec![(
                "resourceAlreadyRead".to_string(),
                "false".to_string()
            )])
        );
    }
}
