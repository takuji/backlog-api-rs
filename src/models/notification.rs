use chrono::{DateTime, Utc};
use serde::Deserialize;

use super::de::de_optional_scalar_as_string;
use super::{Issue, Project, User};

/// 通知の理由。
///
/// **一次資料は Backlog サーバーの実装**(`NotificationReason.scala`)。
/// 公式ドキュメントの一覧は不完全で、16 と 18 の記載が無い。
/// サーバーが定義しているのは 1-6 / 9 / 10-18 で、**7 と 8 は存在しない**
/// (サーバーの `apply` が例外を投げるため API から返り得ない)。
/// 0(`NoReason`)は定義されているがサーバーの `values` / `apply` から
/// 除外されているので、これも返らない。
///
/// コードは後から追加される(14 / 15 / 17 はドキュメント機能とともに増えた)。
/// 未知のコードは `Other` にコードを保持したまま落とし、情報を失わない。
///
/// **`#[non_exhaustive]` を付けないこと。** この enum は意図的に non-exhaustive
/// にしていない。理由コードを追加したとき、`backlog-mcp` 側の日本語ラベルの
/// `match`(`backlog-mcp/src/tools/notifications.rs` の `reason_description`)を
/// **コンパイルエラーで**強制的に更新させるための仕掛けである。
/// `#[non_exhaustive]` を付けると `backlog-mcp` 側は「パターン `_` が
/// 網羅されていない」というエラーになり、その場しのぎで `_ =>` を足すと
/// この仕掛けが恒久的に無効化される。新しいコードは `Other(u32)` で
/// ランタイムの情報を失わずに受けられるので、コンパイル時の強制がなくなっても
/// 動作は壊れない分、気づかれないまま放置されるおそれがある。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NotificationReason {
    /// 1: 課題の担当者に設定された
    AssignedToIssue,
    /// 2: 課題にコメントがあった
    IssueCommented,
    /// 3: 課題が追加された
    IssueCreated,
    /// 4: 課題が更新された
    IssueUpdated,
    /// 5: ファイルが追加された
    FileAdded,
    /// 6: プロジェクトメンバーに追加された
    ProjectUserAdded,
    /// 10: プルリクエストの担当者に設定された
    AssignedToPullRequest,
    /// 11: プルリクエストにコメントがあった
    PullRequestCommented,
    /// 12: プルリクエストが追加された
    PullRequestAdded,
    /// 13: プルリクエストが更新された
    PullRequestUpdated,
    /// 14: ドキュメントにコメントがあった
    DocumentCommented,
    /// 15: ドキュメントのコメントに返信があった
    DocumentCommentReplied,
    /// 16: 課題が一括登録され、その担当者になった(サーバー実装の `IssueMultiCreated`)。
    ///
    /// 一括登録はプロジェクト単位の活動なので、この通知は個別の課題を指さない
    /// (実APIでも `issue` は `null` で、`project` と `sender` のみを持つ)。
    IssueMultiCreated,
    /// 17: ドキュメントでメンションされた
    DocumentMentioned,
    /// 18: 課題の一括登録でお知らせ先に指定された
    /// (サーバー実装の `IssueMultiCreatedNotified`)。
    ///
    /// 16 との違いは受け取る理由。16 は一括登録された課題の**担当者**に送られ、
    /// 18 は**お知らせ先として明示的に指定されたユーザー**に送られる
    /// (担当者として通知される人はサーバー側で除外される)。
    IssueMultiCreatedNotified,
    /// 9(公式に「その他」)と、未知のコードすべて。
    Other(u32),
}

/// 通知に含まれるコメント。
///
/// `comment` / `pull_request_comment` / `document_comment` /
/// `document_comment_reply` の4フィールド共通の入れ物。
///
/// 既存の [`super::Comment`] は `project_id` / `issue_id` / `created_user` を必須で
/// 持つため流用しない。ドキュメントへのコメント(理由 14 / 15)は課題に属さず
/// これらを持ち得ない。通知一覧という1つの応答の中に1件でも形の合わない要素が
/// あれば全体が失敗するため、必須フィールドを最小限にする。
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NotificationComment {
    /// コメントID。数値・文字列のどちらでも受ける。
    ///
    /// Backlog は同じ「ID」という概念をリソースによって数値と文字列で使い分ける
    /// (`Comment.id` は数値だが `DocumentComment.id` / `Document.id` は文字列)。
    /// スカラーIDの型を1つに決め打ちすると、想定外の型が1件混ざるだけで
    /// 応答全体のデシリアライズが失敗するため、寛容に受ける。
    ///
    /// `comment`(課題のコメント)では数値、`document_comment` /
    /// `document_comment_reply`(ドキュメントのコメント)では文字列が来る。
    /// この型は両方の入れ物として使われるため、どちらでも受ける必要がある。
    #[serde(default, deserialize_with = "de_optional_scalar_as_string")]
    pub id: Option<String>,
    /// 書式付きの本文。
    #[serde(default)]
    pub content: Option<String>,
    /// プレーンテキストの本文。
    ///
    /// ドキュメントのコメントは `content`(書式付き)と `plain` の両方を持つ。
    /// 課題のコメントは `content` だけなので、こちらは `None` になる。
    /// 抜粋を作るときは `plain` を優先する。
    #[serde(default)]
    pub plain: Option<String>,
}

/// 通知に含まれるドキュメント。
///
/// 既存の [`super::Document`] は `project_id` / `title` / `status_id` /
/// `created_user` / `created` を必須で持つ。理由17の通知に埋め込まれた
/// ドキュメントは実APIで観測した限りこれらをすべて持っていたが、理由 14 / 15 の
/// 形状は未観測であり、必須フィールドの多いモデルを流用すると1件でも形が
/// 違えば通知一覧の応答全体が失敗する。
///
/// 通知の表示に必要なのはタイトルとIDだけで、本文が必要なら `id` を
/// `get_document` に渡せばよいため、最小限にとどめる。
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NotificationDocument {
    #[serde(default, deserialize_with = "de_optional_scalar_as_string")]
    pub id: Option<String>,
    #[serde(default)]
    pub title: Option<String>,
}

/// 通知に含まれるプルリクエスト。
///
/// Git / プルリクエストのドメイン全体は型付けしない。通知の表示に必要な
/// 2フィールドのみ(Tauriアプリがこの2つで足りることを実証している)。
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NotificationPullRequest {
    #[serde(default)]
    pub number: Option<u64>,
    #[serde(default)]
    pub summary: Option<String>,
}

/// 通知に含まれるGitリポジトリ。
///
/// プルリクエスト関連の通知(理由 10〜13)で、どのリポジトリのPRかを示す。
/// プルリクエストのエンドポイントはプロジェクトキーとリポジトリ名を要求するため、
/// **これが無いと通知からPRへたどれない。**
///
/// openapi.yaml の `SimpleGitRepository` は `{id, name, description}` だが、
/// 通知の表示に必要なのは名前だけなので最小限にとどめる。
/// **理由10〜13の通知は実APIで観測できていない**ため、全フィールド `Option`。
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NotificationRepository {
    #[serde(default)]
    pub id: Option<u64>,
    #[serde(default)]
    pub name: Option<String>,
}

/// Backlogの通知(お知らせ)。
///
/// ネストしたリソースはすべて `Option`。openapi.yaml はこれらを required と
/// しているが、公式ドキュメントの応答例では `repository` / `pullRequest` /
/// `pullRequestComment` が null であり、プロジェクトメンバー追加の通知
/// (理由 6)には課題もコメントも無い。
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Notification {
    pub id: u64,
    pub already_read: bool,
    /// 生の理由コード。意味は [`Notification::reason_kind`] で得る。
    pub reason: u32,
    pub resource_already_read: bool,
    #[serde(default)]
    pub project: Option<Project>,
    #[serde(default)]
    pub issue: Option<Issue>,
    #[serde(default)]
    pub comment: Option<NotificationComment>,
    #[serde(default)]
    pub pull_request: Option<NotificationPullRequest>,
    #[serde(default)]
    pub pull_request_comment: Option<NotificationComment>,
    /// プルリクエスト関連の通知(理由 10〜13)のリポジトリ。
    ///
    /// 課題・ドキュメントの通知では `null`(実APIで観測済み)。
    #[serde(default)]
    pub repository: Option<NotificationRepository>,
    #[serde(default)]
    pub sender: Option<User>,
    /// ドキュメント関連の通知(理由 14 / 15 / 17)の対象ドキュメント。
    ///
    /// **`openapi.yaml` と公式APIリファレンスにはこのフィールドの記載が無い。**
    /// 実APIが返すことを通知100件の調査で確認し、公式TSクライアント
    /// `backlog-js` の `entity.ts` にも宣言がある
    /// (`backlog-mcp/docs/openapi-yaml-discrepancies.md` の #11)。
    #[serde(default)]
    pub document: Option<NotificationDocument>,
    /// ドキュメントへのコメント(理由 14)。`comment` とは別のフィールド。
    ///
    /// ドキュメント関連の通知は `comment` を使わず `null` にする
    /// (理由17で実測)。こちらも未文書のフィールド。
    #[serde(default)]
    pub document_comment: Option<NotificationComment>,
    /// ドキュメントのコメントへの返信(理由 15)。こちらも未文書のフィールド。
    #[serde(default)]
    pub document_comment_reply: Option<NotificationComment>,
    pub created: DateTime<Utc>,
}

impl Notification {
    /// 理由コードを意味に変換する。未知のコードは `Other` に保持される。
    pub fn reason_kind(&self) -> NotificationReason {
        match self.reason {
            1 => NotificationReason::AssignedToIssue,
            2 => NotificationReason::IssueCommented,
            3 => NotificationReason::IssueCreated,
            4 => NotificationReason::IssueUpdated,
            5 => NotificationReason::FileAdded,
            6 => NotificationReason::ProjectUserAdded,
            10 => NotificationReason::AssignedToPullRequest,
            11 => NotificationReason::PullRequestCommented,
            12 => NotificationReason::PullRequestAdded,
            13 => NotificationReason::PullRequestUpdated,
            14 => NotificationReason::DocumentCommented,
            15 => NotificationReason::DocumentCommentReplied,
            16 => NotificationReason::IssueMultiCreated,
            17 => NotificationReason::DocumentMentioned,
            18 => NotificationReason::IssueMultiCreatedNotified,
            // 9(公式に「その他」)と未知のコード。コードを保持する。
            other => NotificationReason::Other(other),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn maps_known_reason_codes() {
        for (code, expected) in [
            (1, NotificationReason::AssignedToIssue),
            (2, NotificationReason::IssueCommented),
            (3, NotificationReason::IssueCreated),
            (4, NotificationReason::IssueUpdated),
            (5, NotificationReason::FileAdded),
            (6, NotificationReason::ProjectUserAdded),
            (10, NotificationReason::AssignedToPullRequest),
            (11, NotificationReason::PullRequestCommented),
            (12, NotificationReason::PullRequestAdded),
            (13, NotificationReason::PullRequestUpdated),
            (14, NotificationReason::DocumentCommented),
            (15, NotificationReason::DocumentCommentReplied),
            (16, NotificationReason::IssueMultiCreated),
            (17, NotificationReason::DocumentMentioned),
            (18, NotificationReason::IssueMultiCreatedNotified),
        ] {
            let n = notification_with_reason(code);
            assert_eq!(n.reason_kind(), expected, "code {code}");
        }
    }

    /// 未知のコードはコードを保持して `Other` に落ちること。
    ///
    /// 14 / 15 / 17 は後から追加された。同じことが再び起きたときに
    /// 「その他」に丸めてコードを失うと、原因の調査ができなくなる。
    #[test]
    fn unknown_reason_codes_keep_their_number() {
        // 9 は公式に「その他」
        assert_eq!(
            notification_with_reason(9).reason_kind(),
            NotificationReason::Other(9)
        );
        // サーバー実装に存在しないコード。7 / 8 はサーバーの `apply` が例外を
        // 投げるので API から返り得ず、0(`NoReason`)も `values` / `apply` から
        // 除外されている。19 以降は将来の追加ぶん。いずれも到達しないはずだが、
        // 万一届いてもコードを保持したまま落ちることを固定する。
        for code in [0, 7, 8, 19, 99] {
            assert_eq!(
                notification_with_reason(code).reason_kind(),
                NotificationReason::Other(code),
                "code {code}"
            );
        }
    }

    fn notification_with_reason(reason: u32) -> Notification {
        serde_json::from_str(&format!(
            r#"{{ "id": 1, "alreadyRead": false, "reason": {reason},
                  "resourceAlreadyRead": false, "created": "2013-10-31T06:58:59Z" }}"#
        ))
        .unwrap()
    }

    /// ネストしたリソースがすべて欠けていてもデシリアライズが通ること。
    ///
    /// プロジェクトメンバー追加の通知(理由 6)がこの形になる。openapi.yaml は
    /// `issue` / `comment` / `project` / `pullRequest` / `pullRequestComment` を
    /// required としているが、それを信じると一覧が丸ごと取れなくなる。
    #[test]
    fn deserializes_notification_without_any_nested_resource() {
        let n = notification_with_reason(6);
        assert!(n.project.is_none());
        assert!(n.issue.is_none());
        assert!(n.comment.is_none());
        assert!(n.pull_request.is_none());
        assert!(n.pull_request_comment.is_none());
        assert!(n.sender.is_none());
    }

    /// ネストしたリソースが明示的に null でもデシリアライズが通ること。
    ///
    /// 公式ドキュメントの応答例は `repository` / `pullRequest` /
    /// `pullRequestComment` を null で返している(欠落ではなく null)。
    #[test]
    fn deserializes_notification_with_explicit_nulls() {
        let n: Notification = serde_json::from_str(
            r#"{ "id": 1, "alreadyRead": false, "reason": 6,
                 "resourceAlreadyRead": false,
                 "project": null, "issue": null, "comment": null,
                 "repository": null, "pullRequest": null, "pullRequestComment": null,
                 "sender": null,
                 "created": "2013-10-31T06:58:59Z" }"#,
        )
        .unwrap();
        assert!(n.issue.is_none());
        assert!(n.pull_request.is_none());
    }

    /// `comment` が `id` と `content` だけの最小形でも受けられること。
    ///
    /// 既存の `Comment` は `projectId` / `issueId` / `createdUser` を必須で持つため、
    /// 流用していたらこの形は必須フィールド不足で失敗する。それを避けるために
    /// 最小限の [`NotificationComment`] を定義しているという設計判断を固定する。
    ///
    /// 理由14(ドキュメントへのコメント)を例に使っているが、**理由14の実際の形状は
    /// 未観測**である(実APIで観測できた理由17は `comment` ではなく未文書の
    /// 専用フィールドを使っていた)。ここで検証しているのは理由と形状の対応ではなく、
    /// 必須フィールドの少ないモデルであることそのもの。
    #[test]
    fn deserializes_document_comment_notification() {
        let n: Notification = serde_json::from_str(
            r#"{ "id": 2, "alreadyRead": false, "reason": 14,
                 "resourceAlreadyRead": false,
                 "comment": { "id": 900, "content": "ドキュメントへのコメント" },
                 "created": "2026-07-30T00:00:00Z" }"#,
        )
        .unwrap();
        assert_eq!(n.reason_kind(), NotificationReason::DocumentCommented);
        assert_eq!(
            n.comment.unwrap().content.as_deref(),
            Some("ドキュメントへのコメント")
        );
    }

    /// ドキュメント関連の通知(理由 17)を実APIで観測した形で受けられること。
    ///
    /// 実APIの通知100件を調べた結果、理由17は `comment` を `null` にして
    /// `document` にドキュメントを入れていた。埋め込まれたドキュメントの
    /// `json` は **`null`** で返っており、公式TSクライアント `backlog-js` が
    /// `json: string`(非optional)と宣言しているのと食い違う。最小限の
    /// [`NotificationDocument`] しか使わないので影響は受けないが、既存の
    /// [`super::Document`] を流用していたらここが危険だった。
    #[test]
    fn deserializes_document_mentioned_notification() {
        let n: Notification = serde_json::from_str(
            r#"{ "id": 1, "alreadyRead": false, "reason": 17,
                 "resourceAlreadyRead": false,
                 "comment": null,
                 "document": { "id": "019343454047a1adb2113d7792bb4351",
                               "projectId": 2, "title": "設計メモ",
                               "json": null, "plain": "本文", "emoji": null,
                               "statusId": 1, "attachments": [], "tags": [],
                               "childDocumentIds": [] },
                 "documentComment": null, "documentCommentReply": null,
                 "created": "2026-07-31T00:00:00Z" }"#,
        )
        .unwrap();
        assert_eq!(n.reason_kind(), NotificationReason::DocumentMentioned);
        let doc = n.document.expect("document があるはず");
        assert_eq!(doc.id.as_deref(), Some("019343454047a1adb2113d7792bb4351"));
        assert_eq!(doc.title.as_deref(), Some("設計メモ"));
        assert!(n.comment.is_none());
    }

    /// ドキュメントへのコメント(理由 14)と返信(理由 15)を受けられること。
    ///
    /// **この2つの形状は未観測である。** 実APIで観測できたのは理由17だけで、
    /// そこでは `documentComment` / `documentCommentReply` はどちらも `null`
    /// だった。したがってフィールド名は `backlog-js` の `entity.ts` を根拠に
    /// しており、中身は必須フィールドを持たない形で受ける。1件でも形が
    /// 合わなければ一覧全体が失敗するため、推測の範囲を最小にしている。
    #[test]
    fn deserializes_document_comment_and_reply_notifications() {
        let commented: Notification = serde_json::from_str(
            r#"{ "id": 2, "alreadyRead": false, "reason": 14,
                 "resourceAlreadyRead": false,
                 "document": { "id": "abc", "title": "議事録" },
                 "documentComment": { "id": "cmt-1", "content": "<p>書式付き</p>",
                                      "plain": "プレーン本文" },
                 "created": "2026-07-31T00:00:00Z" }"#,
        )
        .unwrap();
        assert_eq!(
            commented.reason_kind(),
            NotificationReason::DocumentCommented
        );
        let c = commented
            .document_comment
            .expect("documentComment があるはず");
        assert_eq!(c.id.as_deref(), Some("cmt-1"));
        assert_eq!(c.plain.as_deref(), Some("プレーン本文"));
        assert_eq!(c.content.as_deref(), Some("<p>書式付き</p>"));

        let replied: Notification = serde_json::from_str(
            r#"{ "id": 3, "alreadyRead": false, "reason": 15,
                 "resourceAlreadyRead": false,
                 "documentCommentReply": { "id": "rep-1", "plain": "返信本文" },
                 "created": "2026-07-31T00:00:00Z" }"#,
        )
        .unwrap();
        assert_eq!(
            replied.reason_kind(),
            NotificationReason::DocumentCommentReplied
        );
        assert_eq!(
            replied.document_comment_reply.unwrap().plain.as_deref(),
            Some("返信本文")
        );
    }

    /// ドキュメント系の3フィールドが欠落・null のどちらでも通ること。
    ///
    /// 課題やプルリクエストの通知にはこれらが無い(または null)ので、
    /// 全通知がこの経路を通る。
    #[test]
    fn document_fields_tolerate_absence_and_null() {
        let absent: Notification = serde_json::from_str(
            r#"{ "id": 4, "alreadyRead": false, "reason": 2,
                 "resourceAlreadyRead": false,
                 "created": "2026-07-31T00:00:00Z" }"#,
        )
        .unwrap();
        assert!(absent.document.is_none());
        assert!(absent.document_comment.is_none());
        assert!(absent.document_comment_reply.is_none());

        let nulls: Notification = serde_json::from_str(
            r#"{ "id": 5, "alreadyRead": false, "reason": 2,
                 "resourceAlreadyRead": false,
                 "document": null, "documentComment": null,
                 "documentCommentReply": null,
                 "created": "2026-07-31T00:00:00Z" }"#,
        )
        .unwrap();
        assert!(nulls.document.is_none());
        assert!(nulls.document_comment.is_none());
        assert!(nulls.document_comment_reply.is_none());
    }

    /// `comment.id` が数値・文字列・null・欠落のいずれでもデシリアライズが通ること。
    ///
    /// 課題へのコメント(理由 1 / 2 / 3 / 4)が数値IDを返すことは実APIで確認済み。
    /// 文字列IDがこのフィールドに来る経路は**未観測**だが、Backlog は
    /// `DocumentComment.id` / `Document.id` を文字列で返すなど、スカラーIDの型を
    /// リソースによって使い分けるため、両方を受けられることを固定しておく。
    #[test]
    fn comment_id_accepts_number_string_null_and_missing() {
        let numeric: NotificationComment =
            serde_json::from_str(r#"{ "id": 900, "content": "x" }"#).unwrap();
        assert_eq!(numeric.id.as_deref(), Some("900"));

        let stringy: NotificationComment =
            serde_json::from_str(r#"{ "id": "019343454047a1adb2113d7792bb4351", "content": "x" }"#)
                .unwrap();
        assert_eq!(
            stringy.id.as_deref(),
            Some("019343454047a1adb2113d7792bb4351")
        );

        let null_id: NotificationComment =
            serde_json::from_str(r#"{ "id": null, "content": "x" }"#).unwrap();
        assert_eq!(null_id.id, None);

        let missing_id: NotificationComment =
            serde_json::from_str(r#"{ "content": "x" }"#).unwrap();
        assert_eq!(missing_id.id, None);
    }

    /// 一覧の中に数値IDと文字列IDのコメントが混在しても、両方ともデシリアライズ
    /// できること。**一覧は「1件でも形が合わなければ全体が失敗する」構造なので、
    /// この巻き添えを起こさないことを固定するのがこのテストの目的。**
    ///
    /// `NotificationComment.id` が `Option<u64>` だったころは、文字列IDを持つ要素が
    /// 1件混じると `invalid type: string "0193…", expected u64` で全体が失敗し、
    /// 数値IDの要素(1件目)まで失われていた。
    ///
    /// なお実APIの通知100件を調べた時点では、文字列IDを持つ通知は観測できていない
    /// (ドキュメント関連の通知は `comment` を使わず `null` を返していた)。
    /// このテストは実在する形状の再現ではなく、**混在に対する耐性そのもの**の検証である。
    #[test]
    fn deserializes_mixed_list_with_numeric_and_string_comment_ids() {
        let json = r##"[
            { "id": 1, "alreadyRead": false, "reason": 2,
              "resourceAlreadyRead": false,
              "comment": { "id": 7007, "content": "課題へのコメント" },
              "created": "2013-10-31T06:58:59Z" },
            { "id": 2, "alreadyRead": false, "reason": 14,
              "resourceAlreadyRead": false,
              "comment": { "id": "019343454047a1adb2113d7792bb4351", "content": "ドキュメントへのコメント" },
              "created": "2026-07-30T00:00:00Z" }
        ]"##;
        let list: Vec<Notification> = serde_json::from_str(json).unwrap();
        assert_eq!(list.len(), 2);
        assert_eq!(
            list[0].comment.as_ref().unwrap().id.as_deref(),
            Some("7007")
        );
        assert_eq!(
            list[1].comment.as_ref().unwrap().id.as_deref(),
            Some("019343454047a1adb2113d7792bb4351")
        );
    }

    /// 公式ドキュメント(get-notification)の応答例をそのままデシリアライズできること。
    ///
    /// fixture は https://developer.nulab.com/docs/backlog/api/2/get-notification/
    /// の応答例を書き起こさずそのまま使っている。自作 fixture でテストが通るのに
    /// 実APIで失敗する事故を防ぐため。
    #[test]
    fn deserializes_the_official_response_example() {
        let list: Vec<Notification> = serde_json::from_str(OFFICIAL_EXAMPLE).unwrap();
        assert_eq!(list.len(), 1);
        let n = &list[0];
        assert_eq!(n.id, 299);
        assert_eq!(n.reason_kind(), NotificationReason::IssueCommented);
        assert_eq!(n.issue.as_ref().unwrap().issue_key, "TEST2-17");
        assert_eq!(n.comment.as_ref().unwrap().content.as_deref(), Some("hoge"));
        assert_eq!(n.sender.as_ref().unwrap().name, "eguchi");
        // 応答例では null
        assert!(n.pull_request.is_none());
        assert!(n.pull_request_comment.is_none());
    }

    /// 通知に埋め込まれた課題の配列フィールドが null でもデシリアライズが通り、
    /// 空の `Vec` になること(`null_as_empty_vec` が通知経由でも効くことのピン留め)。
    ///
    /// 公式応答例([`OFFICIAL_EXAMPLE`])の `issue` は配列がすべて `[]` であり、
    /// null 耐性を一切検証できない。実APIは空コレクションを `null` で返すことが
    /// あるため、通知に埋め込まれた課題でも同じことが起こり得る想定で組む。
    #[test]
    fn deserializes_notification_with_null_arrays_in_embedded_issue() {
        let n: Notification = serde_json::from_str(
            r##"{ "id": 3, "alreadyRead": false, "reason": 2,
                 "resourceAlreadyRead": false,
                 "issue": {
                     "id": 4531, "projectId": 2, "issueKey": "TEST2-17", "keyId": 17,
                     "issueType": { "id": 7, "projectId": 2, "name": "Bug",
                                    "color": "#990000", "displayOrder": 0 },
                     "summary": "aaa",
                     "status": { "id": 1, "projectId": 2, "name": "Open",
                                 "color": "#ed8077", "displayOrder": 1000 },
                     "createdUser": { "id": 1, "userId": "admin", "name": "admin", "roleType": 1 },
                     "created": "2013-04-23T07:38:59Z",
                     "category": null, "versions": null, "milestone": null,
                     "customFields": null, "attachments": null, "sharedFiles": null,
                     "stars": null
                 },
                 "created": "2013-10-31T06:58:59Z" }"##,
        )
        .unwrap();
        let issue = n.issue.as_ref().unwrap();
        assert!(issue.category.is_empty());
        assert!(issue.versions.is_empty());
        assert!(issue.milestone.is_empty());
        assert!(issue.custom_fields.is_empty());
        assert!(issue.attachments.is_empty());
        assert!(issue.shared_files.is_empty());
        assert!(issue.stars.is_empty());
    }

    /// `NotificationPullRequest` と `NotificationComment` の最小形(全フィールド欠落)を
    /// 受けられること。
    ///
    /// `pullRequest` に `number` / `summary` を、`comment` に `id` を欠いたオブジェクトを
    /// 与える fixture がこれまで無く、両型の `Option` 化が無検証だった
    /// (`u64` / `String` に変えても全テストが通っていた)。
    #[test]
    fn deserializes_minimal_pull_request_and_comment() {
        let n: Notification = serde_json::from_str(
            r#"{ "id": 4, "alreadyRead": false, "reason": 11,
                 "resourceAlreadyRead": false,
                 "pullRequest": {},
                 "comment": {},
                 "created": "2013-10-31T06:58:59Z" }"#,
        )
        .unwrap();
        let pr = n.pull_request.as_ref().unwrap();
        assert!(pr.number.is_none());
        assert!(pr.summary.is_none());
        assert!(n.comment.as_ref().unwrap().id.is_none());
    }

    /// 通知の `repository` を受けられること(欠落・null・値ありのすべて)。
    ///
    /// プルリクエスト関連の通知(理由 10〜13)から `get_pull_request` へたどるには
    /// リポジトリ名が必要になる。
    ///
    /// **理由10〜13の通知は実APIで観測できていない**(調べた100件に含まれていな
    /// かった)。フィールド名の根拠は openapi.yaml の `SimpleGitRepository`
    /// (`{id, name, description}`)であり、中身は全フィールド `Option` で受ける。
    #[test]
    fn deserializes_notification_repository() {
        let with_repo: Notification = serde_json::from_str(
            r#"{ "id": 1, "alreadyRead": false, "reason": 11,
                 "resourceAlreadyRead": false,
                 "repository": { "id": 3, "name": "example-repo" },
                 "pullRequest": { "number": 12, "summary": "設定を見直す" },
                 "created": "2026-08-01T00:00:00Z" }"#,
        )
        .unwrap();
        let repo = with_repo.repository.expect("repository があるはず");
        assert_eq!(repo.id, Some(3));
        assert_eq!(repo.name.as_deref(), Some("example-repo"));

        // 課題やドキュメントの通知では null(実APIで観測済み)
        let null_repo: Notification = serde_json::from_str(
            r#"{ "id": 2, "alreadyRead": false, "reason": 2,
                 "resourceAlreadyRead": false, "repository": null,
                 "created": "2026-08-01T00:00:00Z" }"#,
        )
        .unwrap();
        assert!(null_repo.repository.is_none());

        let absent: Notification = serde_json::from_str(
            r#"{ "id": 3, "alreadyRead": false, "reason": 2,
                 "resourceAlreadyRead": false,
                 "created": "2026-08-01T00:00:00Z" }"#,
        )
        .unwrap();
        assert!(absent.repository.is_none());
    }

    /// https://developer.nulab.com/docs/backlog/api/2/get-notification/ の応答例。
    const OFFICIAL_EXAMPLE: &str = r##"[
    {
        "id": 299,
        "alreadyRead": true,
        "reason": 2,
        "resourceAlreadyRead": true,
        "project": {
            "id": 2,
            "projectKey": "TEST2",
            "name": "test2",
            "chartEnabled": true,
            "useResolvedForChart": true,
            "subtaskingEnabled": true,
            "projectLeaderCanEditProjectLeader": false,
            "useWiki": true,
            "useDocument": true,
            "useFileSharing": true,
            "useWikiTreeView": true,
            "useSubversion": true,
            "useGit": true,
            "useOriginalImageSizeAtWiki": false,
            "textFormattingRule": "backlog",
            "archived": false,
            "displayOrder": 3,
            "useDevAttributes": true
        },
        "issue": {
            "id": 4531,
            "projectId": 2,
            "issueKey": "TEST2-17",
            "keyId": 17,
            "issueType": {
                "id": 7,
                "projectId": 2,
                "name": "Bug",
                "color": "#990000",
                "displayOrder": 0
            },
            "summary": "aaa",
            "description": "",
            "resolution": null,
            "priority": {
                "id": 3,
                "name": "Normal"
            },
            "status": {
                "id": 1,
                "projectId": 2,
                "name": "Open",
                "color": "#ed8077",
                "displayOrder": 1000
            },
            "assignee": {
                "id": 2,
                "userId": "eguchi",
                "name": "eguchi",
                "roleType": 2,
                "lang": null,
                "nulabAccount": {
                    "nulabId": "tSaVeJfRxLURSAkgfbNAfCbM7PqddYLJ3nG3BELjx6eSTbu8LD",
                    "name": "eguchi",
                    "uniqueId": "eguchi"
                },
                "mailAddress": "eguchi@nulab.example",
                "lastLoginTime": "2022-09-01T06:35:39Z"
            },
            "category": [],
            "versions": [],
            "milestone": [],
            "startDate": "2013-08-29T15:00:00Z",
            "dueDate": "2013-09-03T15:00:00Z",
            "estimatedHours": null,
            "actualHours": null,
            "parentIssueId": null,
            "createdUser": {
                "id": 1,
                "userId": "admin",
                "name": "admin",
                "roleType": 1,
                "lang": "ja",
                "nulabAccount": {
                    "nulabId": "Prm9ZD9DQD5snNWcSYSwZiQoA9WFBUEa2ySznrSnSQRhdC2X8G",
                    "name": "admin",
                    "uniqueId": "admin"
                },
                "mailAddress": "eguchi@nulab.example",
                "lastLoginTime": "2022-09-01T06:35:39Z"
            },
            "created": "2013-04-23T07:38:59Z",
            "updatedUser": {
                "id": 1,
                "userId": "admin",
                "name": "admin",
                "roleType": 1,
                "lang": "ja",
                "nulabAccount": {
                    "nulabId": "Prm9ZD9DQD5snNWcSYSwZiQoA9WFBUEa2ySznrSnSQRhdC2X8G",
                    "name": "admin",
                    "uniqueId": "admin"
                },
                "mailAddress": "eguchi@nulab.example",
                "lastLoginTime": "2022-09-01T06:35:39Z"
            },
            "updated": "2013-09-06T09:25:41Z",
            "customFields": [],
            "attachments": [],
            "sharedFiles": [],
            "stars": []
        },
        "comment": {
            "id": 7007,
            "projectId": 5,
            "issueId": 50,
            "content": "hoge",
            "changeLog": null,
            "createdUser": {
                "id": 2,
                "userId": "eguchi",
                "name": "eguchi",
                "roleType": 2,
                "lang": null,
                "nulabAccount": {
                    "nulabId": "tSaVeJfRxLURSAkgfbNAfCbM7PqddYLJ3nG3BELjx6eSTbu8LD",
                    "name": "eguchi",
                    "uniqueId": "eguchi"
                },
                "mailAddress": "eguchi@nulab.example",
                "lastLoginTime": "2022-09-01T06:35:39Z"
            },
            "created": "2013-10-31T06:58:58Z",
            "updated": "2013-10-31T06:58:58Z",
            "stars": [],
            "notifications": []
        },
        "repository": null,
        "pullRequest": null,
        "pullRequestComment": null,
        "sender": {
            "id": 2,
            "userId": "eguchi",
            "name": "eguchi",
            "roleType": 2,
            "lang": null,
            "nulabAccount": {
                "nulabId": "tSaVeJfRxLURSAkgfbNAfCbM7PqddYLJ3nG3BELjx6eSTbu8LD",
                "name": "eguchi",
                "uniqueId": "eguchi"
            },
            "mailAddress": "eguchi@nulab.example",
            "lastLoginTime": "2022-09-01T06:35:39Z"
        },
        "created": "2013-10-31T06:58:59Z"
    }
]"##;
}
