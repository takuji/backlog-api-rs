use serde::Deserialize;

use crate::models::Notification;
use crate::params::QueryPairs;
use crate::{BacklogClient, Result};

use super::Order;

/// お知らせ関連API。`client.notifications()` で取得する。
#[derive(Debug, Clone, Copy)]
pub struct NotificationsApi<'a> {
    client: &'a BacklogClient,
}

/// お知らせ一覧の取得(GET /api/v2/notifications)のパラメータ。
///
/// <https://developer.nulab.com/docs/backlog/api/2/get-notification/>
///
/// openapi.yaml にはこのエンドポイントのクエリパラメータが1つも記載されていないが、
/// 公式ドキュメントには minId / maxId / count / order / senderId が載っており、
/// Tauriアプリが count と maxId で実際に動作している。
#[derive(Debug, Clone, Default)]
pub struct GetNotificationsParams {
    pairs: QueryPairs,
}

impl GetNotificationsParams {
    pub fn new() -> Self {
        Self::default()
    }

    /// 取得件数(1-100、APIの既定は20)。
    pub fn count(mut self, count: u64) -> Self {
        self.pairs.push("count", count);
        self
    }

    /// このIDより新しい通知に絞る。
    pub fn min_id(mut self, id: u64) -> Self {
        self.pairs.push("minId", id);
        self
    }

    /// このIDより古い通知に絞る(古い通知をたどるページングに使う)。
    pub fn max_id(mut self, id: u64) -> Self {
        self.pairs.push("maxId", id);
        self
    }

    /// 並び順(APIの既定は desc)。
    pub fn order(mut self, order: Order) -> Self {
        self.pairs.push("order", order.as_str());
        self
    }

    /// 送信者のユーザーIDで絞る。
    pub fn sender_id(mut self, id: u64) -> Self {
        self.pairs.push("senderId", id);
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

impl<'a> NotificationsApi<'a> {
    pub(crate) fn new(client: &'a BacklogClient) -> Self {
        Self { client }
    }

    /// 自分に届いた通知を取得する。既定は新しい順(`params` の `order` で変えられる)。
    ///
    /// 壊れた要素があっても呼び出し全体は失敗せず、
    /// [`PartialList::skipped`] に落とした事実が入る。通知は種類が多く
    /// 未文書のフィールドも実在するため、1件の想定外でページ全体が
    /// 空になるのを避ける。
    pub async fn list(
        &self,
        params: GetNotificationsParams,
    ) -> Result<crate::PartialList<Notification>> {
        self.client
            .get_list("/api/v2/notifications", params.into_pairs())
            .await
    }

    /// 通知1件を既読にする。
    ///
    /// 公式表題は **Read Notification**。応答は 204 No Content。
    /// **この操作は取り消せない**(未読へ戻すエンドポイントは存在しない)。
    ///
    /// [`Self::reset_unread_count`] とは別物である。あちらは集計カウンタの
    /// リセットで、個々の通知は未読のまま残る。名前が似ているので注意。
    pub async fn mark_as_read(&self, id: u64) -> Result<()> {
        self.client
            .post_no_content(&crate::client::api_path(&[
                "notifications",
                &id.to_string(),
                "markAsRead",
            ]))
            .await
    }

    /// 「未読の通知がある」という集計状態をリセットし、リセット後の件数を返す。
    ///
    /// 公式表題は **Reset Unread Notification Count**。エンドポイントの綴りは
    /// `POST /notifications/markAsRead` だが、**個々の通知は既読にならない**。
    /// 名前から「全件既読化」と読めてしまうが違う。
    ///
    /// 個々の通知を既読にするのは [`Self::mark_as_read`]。
    pub async fn reset_unread_count(&self) -> Result<u64> {
        let resp: CountResponse = self
            .client
            .post("/api/v2/notifications/markAsRead", QueryPairs::new())
            .await?;
        Ok(resp.count)
    }

    /// 未読の通知件数を取得する(`alreadyRead=false` **のみ**)。
    ///
    /// 「通知そのものが未読」を数える。`resourceAlreadyRead=false` は**足さない**。
    /// 両方を指定すると、Backlog を直接開いて対象の課題を読んだ通知が件数から
    /// 落ちてしまい、「未読件数」という名前から想像できない挙動になる。
    ///
    /// Tauriアプリ backlog-inbox は当初 `resourceAlreadyRead=false` も
    /// 指定していたが、**この定義に揃えることで決着した**(2026-08-01)。
    /// アプリをライブラリに寄せる際、バッジの数字は増える方向に変わる。
    /// この1行を「両方指定」に戻すと定義が分岐するので変更しないこと。
    pub async fn unread_count(&self) -> Result<u64> {
        let mut query = QueryPairs::new();
        query.push("alreadyRead", false);
        let resp: CountResponse = self
            .client
            .get("/api/v2/notifications/count", query)
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
    /// `HashSet<(String, String)>` にはしない: `("minId", "1")` のような
    /// ペア単位で見れば異なるキー・値の取り違えはペアのまま区別できるが、
    /// 「同じペアが1個だけ届くべきところ2個届いた」という多重度だけの違いは
    /// 集合にした時点で潰れてしまい検出できない。ソート済み `Vec` なら
    /// 重複を保持したまま並び順の違いだけを無視できる。
    fn sorted_pairs(mut pairs: Vec<(String, String)>) -> Vec<(String, String)> {
        pairs.sort();
        pairs
    }

    #[tokio::test]
    async fn list_requests_notifications_with_query() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/api/v2/notifications"))
            .respond_with(ResponseTemplate::new(200).set_body_raw(
                r#"[{ "id": 1, "alreadyRead": false, "reason": 2,
                      "resourceAlreadyRead": false,
                      "created": "2013-10-31T06:58:59Z" }]"#,
                "application/json",
            ))
            .expect(1)
            .mount(&server)
            .await;

        let client = BacklogClient::new(&server.uri(), "KEY").unwrap();
        let list = client
            .notifications()
            .list(
                GetNotificationsParams::new()
                    .min_id(1)
                    .max_id(2)
                    .count(3)
                    .order(Order::Asc)
                    .sender_id(4),
            )
            .await
            .unwrap();
        assert_eq!(list.items.len(), 1);
        assert_eq!(list.items[0].id, 1);

        // 5つのパラメータそれぞれが正しい名前・値で送られていること、かつ
        // それ以外のパラメータが混じっていないことを厳密に確認する。
        // (例えば minId と maxId を取り違えても query_param マッチャーだけでは
        // 検出できない)
        assert_eq!(
            sorted_pairs(captured_query_pairs(&server).await),
            sorted_pairs(vec![
                ("minId".to_string(), "1".to_string()),
                ("maxId".to_string(), "2".to_string()),
                ("count".to_string(), "3".to_string()),
                ("order".to_string(), "asc".to_string()),
                ("senderId".to_string(), "4".to_string()),
            ])
        );
    }

    /// 未読件数は `alreadyRead=false` を「だけ」送ること(設計書の決定事項8)。
    ///
    /// Tauriアプリは `resourceAlreadyRead=false` も併せて送っているが、これは
    /// あえて採用していない意図的な差異。将来 Tauri アプリに合わせようとして
    /// パラメータを増やす変更が入ったら、このテストで検知できるようにする。
    #[tokio::test]
    async fn unread_count_filters_by_already_read_false() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/api/v2/notifications/count"))
            .and(query_param("alreadyRead", "false"))
            .respond_with(
                ResponseTemplate::new(200).set_body_raw(r#"{"count": 7}"#, "application/json"),
            )
            .expect(1)
            .mount(&server)
            .await;

        let client = BacklogClient::new(&server.uri(), "KEY").unwrap();
        assert_eq!(client.notifications().unread_count().await.unwrap(), 7);

        // alreadyRead=false 以外のクエリパラメータが一切ないことを確認する。
        assert_eq!(
            sorted_pairs(captured_query_pairs(&server).await),
            sorted_pairs(vec![("alreadyRead".to_string(), "false".to_string())])
        );
    }

    /// 個別の既読化が正しいパスへ POST し、204 を受け取れること。
    ///
    /// `execute` は成功時に必ず JSON を読むため、204 No Content をそのまま
    /// 通すと本文が空でデシリアライズに失敗する。`post_no_content` 経由に
    /// なっていることをここで固定する。
    #[tokio::test]
    async fn mark_as_read_posts_and_accepts_no_content() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/api/v2/notifications/299/markAsRead"))
            .respond_with(ResponseTemplate::new(204))
            .expect(1)
            .mount(&server)
            .await;

        let client = BacklogClient::new(&server.uri(), "KEY").unwrap();
        client.notifications().mark_as_read(299).await.unwrap();
    }

    /// カウンタのリセットが件数を返すこと。
    ///
    /// **個別の既読化とは別のエンドポイントである。** 綴りは似ているが、
    /// こちらは `{"count": N}` を返し、個々の通知は未読のまま残る。
    #[tokio::test]
    async fn reset_unread_count_returns_the_remaining_count() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/api/v2/notifications/markAsRead"))
            .respond_with(
                ResponseTemplate::new(200).set_body_raw(r#"{"count": 4}"#, "application/json"),
            )
            .expect(1)
            .mount(&server)
            .await;

        let client = BacklogClient::new(&server.uri(), "KEY").unwrap();
        assert_eq!(
            client.notifications().reset_unread_count().await.unwrap(),
            4
        );
    }

    /// 2つの既読化が**別のパス**を叩くこと。
    ///
    /// 個別(`/notifications/{id}/markAsRead`)と一括のカウンタリセット
    /// (`/notifications/markAsRead`)を取り違えると、意味がまったく変わる。
    /// 片方のモックしか用意しないことで取り違えを検出する。
    #[tokio::test]
    async fn the_two_mark_as_read_endpoints_are_distinct() {
        let server = MockServer::start().await;
        // 個別のパスだけを用意する。カウンタリセットがここへ来たら expect(0) で落ちる
        Mock::given(method("POST"))
            .and(path("/api/v2/notifications/7/markAsRead"))
            .respond_with(ResponseTemplate::new(204))
            .expect(0)
            .mount(&server)
            .await;
        Mock::given(method("POST"))
            .and(path("/api/v2/notifications/markAsRead"))
            .respond_with(
                ResponseTemplate::new(200).set_body_raw(r#"{"count": 0}"#, "application/json"),
            )
            .expect(1)
            .mount(&server)
            .await;

        let client = BacklogClient::new(&server.uri(), "KEY").unwrap();
        client.notifications().reset_unread_count().await.unwrap();
    }

    /// パラメータを指定しない場合はクエリを付けないこと。
    #[tokio::test]
    async fn list_without_params_sends_no_query() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/api/v2/notifications"))
            .respond_with(ResponseTemplate::new(200).set_body_raw("[]", "application/json"))
            .expect(1)
            .mount(&server)
            .await;

        let client = BacklogClient::new(&server.uri(), "KEY").unwrap();
        let list = client
            .notifications()
            .list(GetNotificationsParams::new())
            .await
            .unwrap();
        assert!(list.items.is_empty());

        // クエリパラメータが本当に0個であることを確認する(APIキーはヘッダで
        // 送るため、ここには一切現れない)。
        assert!(captured_query_pairs(&server).await.is_empty());
    }

    /// 通知一覧の1件が壊れていても、残りが返ること。
    ///
    /// 通知は reason が14種以上あり未文書のフィールドも実在するため、
    /// 想定外の形が最も来やすい端点である。
    #[tokio::test]
    async fn list_keeps_the_good_notifications_when_one_is_malformed() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/api/v2/notifications"))
            .respond_with(ResponseTemplate::new(200).set_body_raw(
                r#"[
                  { "id": 1, "alreadyRead": false, "reason": 2,
                    "resourceAlreadyRead": false, "created": "2026-07-30T07:55:07Z" },
                  { "id": 2, "alreadyRead": false },
                  { "id": 3, "alreadyRead": true, "reason": 4,
                    "resourceAlreadyRead": true, "created": "2026-07-29T07:55:07Z" }
                ]"#,
                "application/json",
            ))
            .mount(&server)
            .await;

        let client = BacklogClient::new(&server.uri(), "KEY").unwrap();
        let list = client
            .notifications()
            .list(GetNotificationsParams::new())
            .await
            .unwrap();
        assert_eq!(
            list.items.iter().map(|n| n.id).collect::<Vec<_>>(),
            vec![1, 3]
        );
        assert_eq!(list.skipped.len(), 1);
        assert_eq!(list.skipped[0].index, 1);
        assert_eq!(list.skipped[0].id.as_deref(), Some("2"));
        assert!(
            !list.skipped[0].reason.is_empty(),
            "なぜ読めなかったかが空だと、利用者は原因を追えない"
        );
    }

    /// 全件正常なら skipped は空。
    #[tokio::test]
    async fn list_reports_no_skips_for_a_clean_page() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/api/v2/notifications"))
            .respond_with(ResponseTemplate::new(200).set_body_raw(
                r#"[{ "id": 1, "alreadyRead": false, "reason": 2,
                      "resourceAlreadyRead": false, "created": "2026-07-30T07:55:07Z" }]"#,
                "application/json",
            ))
            .mount(&server)
            .await;

        let client = BacklogClient::new(&server.uri(), "KEY").unwrap();
        let list = client
            .notifications()
            .list(GetNotificationsParams::new())
            .await
            .unwrap();
        assert_eq!(list.items.len(), 1);
        assert!(list.skipped.is_empty());
    }
}
