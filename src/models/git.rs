use chrono::{DateTime, Utc};
use serde::Deserialize;

use super::de::null_as_empty_vec;
use super::{Issue, User};

/// Gitリポジトリ。
///
/// `description` / `hook_url` / `pushed_at` は実データではいずれも `null` で
/// 返ることがある(102件中それぞれ 15 / 41 / 1 件)。openapi.yaml はこの3つを
/// `required` としているが `nullable: true` も付けており、キーが在って値が
/// `null` なのは仕様どおりで誤りではない(`required` は「キーが存在すること」
/// を意味するだけで、「値が非null であること」までは保証しない)。
/// 一方 `description` は公式TSクライアント `backlog-js` が非optionalと
/// 宣言しており、こちらは実態と食い違う本物の誤り。
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GitRepository {
    pub id: u64,
    pub project_id: u64,
    pub name: String,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub hook_url: Option<String>,
    #[serde(default)]
    pub http_url: Option<String>,
    #[serde(default)]
    pub ssh_url: Option<String>,
    #[serde(default)]
    pub display_order: Option<i64>,
    #[serde(default)]
    pub pushed_at: Option<DateTime<Utc>>,
    #[serde(default)]
    pub created_user: Option<User>,
    #[serde(default)]
    pub created: Option<DateTime<Utc>>,
    #[serde(default)]
    pub updated_user: Option<User>,
    #[serde(default)]
    pub updated: Option<DateTime<Utc>>,
}

/// プルリクエストの状態(1=Open, 2=Closed, 3=Merged)。
///
/// **既存の [`super::Status`](課題の状態)を流用してはならない。**
/// 実データの `PullRequest.status` は `{id, name}` だけで、`super::Status` は
/// `project_id` / `color` / `display_order` を必須にしている。流用すると
/// すべてのプルリクエストでデシリアライズが失敗する。
/// IDの意味は Backlog サーバー実装の `PullRequestStatus.scala` で確定している。
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PullRequestStatus {
    pub id: u64,
    #[serde(default)]
    pub name: Option<String>,
}

/// プルリクエスト。
///
/// `assignee` / `issue` / `baseCommit` / `branchCommit` / `mergeCommit` /
/// `closeAt` / `mergeAt` は**実データでは全部 `null` で返ることがある**。
/// ただし openapi.yaml が誤っているのは `assignee` と `issue` だけ
/// (`nullable: true` の指定が無いのに `null` が来る)。`baseCommit` 以下5つは
/// `nullable: true` が付いており、`null` の観測は仕様どおりで openapi の
/// 誤りではない(`required` は「キーが存在すること」を意味するだけで、
/// 「値が非null であること」までは保証しない)。とくに `issue` は100件中51件が
/// `null` で、`backlog-js` が非optionalと宣言しているのは(こちらは)誤り。
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PullRequest {
    pub id: u64,
    pub project_id: u64,
    pub repository_id: u64,
    pub number: u64,
    #[serde(default)]
    pub summary: Option<String>,
    #[serde(default)]
    pub description: Option<String>,
    /// マージ先のブランチ名。
    #[serde(default)]
    pub base: Option<String>,
    /// マージ元のブランチ名。
    #[serde(default)]
    pub branch: Option<String>,
    #[serde(default)]
    pub status: Option<PullRequestStatus>,
    #[serde(default)]
    pub assignee: Option<User>,
    /// 関連課題。**実データでは半数以上が `null`**(課題に紐づかないPRがある)。
    ///
    /// 既存の `Issue` をそのまま流用している。これは意図的なトレードオフである:
    /// 49件の埋め込み課題を観測し、いずれも `Issue` の必須フィールド
    /// (`IssueType` / `Status` とそれぞれの必須フィールドを含む)を満たしていた
    /// ためこの判断を採用した。残るリスクは、想定外の形の埋め込み課題が来た場合に
    /// (例えば `issue.stars` が `[{"id":1}]` だと `missing field url`、
    /// `issue.attachments` が `[{"id":1}]` だと `missing field name`、
    /// `issue.sharedFiles` が `[{"id":1}]` だと `missing field projectId` になるなど、
    /// 埋め込み課題の**ネストした配列**の要素形状がずれた場合)一覧全体の
    /// デシリアライズが失敗し、該当PRを含む一覧が0件になること。
    #[serde(default)]
    pub issue: Option<Issue>,
    #[serde(default)]
    pub base_commit: Option<String>,
    #[serde(default)]
    pub branch_commit: Option<String>,
    #[serde(default)]
    pub merge_commit: Option<String>,
    #[serde(default)]
    pub close_at: Option<DateTime<Utc>>,
    #[serde(default)]
    pub merge_at: Option<DateTime<Utc>>,
    #[serde(default)]
    pub created_user: Option<User>,
    #[serde(default)]
    pub created: Option<DateTime<Utc>>,
    #[serde(default)]
    pub updated_user: Option<User>,
    #[serde(default)]
    pub updated: Option<DateTime<Utc>>,
    /// 未型付け。実データ100件すべてで空配列だったため中身の形状が未観測。
    /// 既存の `Attachment` を流用すると想定外の形で一覧全体が失敗しうる。
    /// 使うのは件数だけ。
    #[serde(default, deserialize_with = "null_as_empty_vec")]
    pub attachments: Vec<serde_json::Value>,
    /// 未型付け。同上(こちらは件数も使わない)。
    #[serde(default, deserialize_with = "null_as_empty_vec")]
    pub stars: Vec<serde_json::Value>,
}

/// プルリクエストのコメント。
///
/// `file_path` / `position` / `old_blob_id` / `new_blob_id` は **openapi.yaml には
/// あるが公式TSクライアント `backlog-js` には無い**。フィールドが実在することは
/// 観測できた(1件のコメントで4つとも返っていた)が、**いずれも `null` だったため
/// 値が入った状態は未観測**である。他の項目(102件・100件など)ほど根拠は強くない。
///
/// ファイル指定のレビューコメントで値が入る**はず**だが、これは openapi.yaml の
/// 記述からの推論であって観測ではない。テストの `inline` fixture も推論に基づく
/// 合成データである。
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PullRequestComment {
    pub id: u64,
    #[serde(default)]
    pub content: Option<String>,
    #[serde(default, deserialize_with = "null_as_empty_vec")]
    pub change_log: Vec<serde_json::Value>,
    /// ファイル指定コメントの対象ファイル。通常のコメントでは `null`
    /// (値が入った状態は未観測。上の型のドキュメント参照)。
    #[serde(default)]
    pub file_path: Option<String>,
    /// ファイル指定コメントの行位置。通常のコメントでは `null`(同上)。
    #[serde(default)]
    pub position: Option<i64>,
    #[serde(default)]
    pub old_blob_id: Option<String>,
    #[serde(default)]
    pub new_blob_id: Option<String>,
    #[serde(default)]
    pub created_user: Option<User>,
    #[serde(default)]
    pub created: Option<DateTime<Utc>>,
    #[serde(default)]
    pub updated: Option<DateTime<Utc>>,
    /// 未型付け。中身の形状が未観測。
    #[serde(default, deserialize_with = "null_as_empty_vec")]
    pub stars: Vec<serde_json::Value>,
    #[serde(default, deserialize_with = "null_as_empty_vec")]
    pub notifications: Vec<serde_json::Value>,
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 実APIで観測した形のリポジトリを受けられること。
    ///
    /// `description` / `hookUrl` / `pushedAt` が `null` の組み合わせは実在する
    /// (102件中それぞれ 15 / 41 / 1 件)。openapi.yaml はこの3つに
    /// `nullable: true` を付けており、この観測を正しく言い当てている。
    /// 観測に基づいて3つとも `Option` にした。
    #[test]
    fn deserializes_repository_with_observed_nulls() {
        let r: GitRepository = serde_json::from_str(
            r#"{ "id": 1, "projectId": 2, "name": "example-repo",
                 "description": null, "hookUrl": null,
                 "httpUrl": "https://example.backlog.com/git/PROJ/example-repo.git",
                 "sshUrl": "example@example.backlog.com:/PROJ/example-repo.git",
                 "displayOrder": 0, "pushedAt": null,
                 "createdUser": { "id": 1, "userId": "admin", "name": "admin", "roleType": 1 },
                 "created": "2013-05-30T09:11:36Z",
                 "updatedUser": { "id": 1, "userId": "admin", "name": "admin", "roleType": 1 },
                 "updated": "2013-05-30T09:11:36Z" }"#,
        )
        .unwrap();
        assert_eq!(r.name, "example-repo");
        assert!(r.description.is_none());
        assert!(r.hook_url.is_none());
        assert!(r.pushed_at.is_none());
        assert_eq!(
            r.http_url.as_deref().unwrap(),
            "https://example.backlog.com/git/PROJ/example-repo.git"
        );
    }

    /// `GitRepository` の任意フィールドが**すべて** `null` でも通ること。
    ///
    /// 個別に固定していくと変更量が大きいので、1つの fixture で
    /// 10フィールド(`description` / `hook_url` / `http_url` / `ssh_url` /
    /// `display_order` / `pushed_at` / `created_user` / `created` /
    /// `updated_user` / `updated`)をまとめて固定する。
    ///
    /// このモデルは**寛容側に倒す**方針である。リポジトリのメタデータ・URL・
    /// 時刻はAPIが `null` を返しても `None` に落ちて劣化するだけで済む。
    /// 逆に将来これらを必須へ「引き締める」変更が入ったとき、テストが無いと
    /// 気づかないままリリースされうるので、ここで方針を固定する。
    #[test]
    fn repository_tolerates_all_optional_fields_being_null() {
        let r: GitRepository = serde_json::from_str(
            r#"{ "id": 1, "projectId": 2, "name": "example-repo",
                 "description": null, "hookUrl": null, "httpUrl": null,
                 "sshUrl": null, "displayOrder": null, "pushedAt": null,
                 "createdUser": null, "created": null,
                 "updatedUser": null, "updated": null }"#,
        )
        .unwrap();
        assert_eq!(r.id, 1);
        assert_eq!(r.name, "example-repo");
        assert!(r.description.is_none());
        assert!(r.hook_url.is_none());
        assert!(r.http_url.is_none());
        assert!(r.ssh_url.is_none());
        assert!(r.display_order.is_none());
        assert!(r.pushed_at.is_none());
        assert!(r.created_user.is_none());
        assert!(r.created.is_none());
        assert!(r.updated_user.is_none());
        assert!(r.updated.is_none());

        // 必須3フィールドが欠けていれば従来どおり失敗する(寛容にしすぎない)
        assert!(
            serde_json::from_str::<GitRepository>(r#"{ "projectId": 2, "name": "x" }"#).is_err()
        );
        assert!(serde_json::from_str::<GitRepository>(r#"{ "id": 1, "name": "x" }"#).is_err());
        assert!(serde_json::from_str::<GitRepository>(r#"{ "id": 1, "projectId": 2 }"#).is_err());
    }

    /// `PullRequestStatus.name` が `null` でも通ること。
    #[test]
    fn pull_request_status_tolerates_null_name() {
        let s: PullRequestStatus = serde_json::from_str(r#"{ "id": 1, "name": null }"#).unwrap();
        assert_eq!(s.id, 1);
        assert!(s.name.is_none());
    }

    /// **関連課題を持たないプルリクエストを受けられること。**
    ///
    /// 実データでは100件中51件が `issue: null` だった。openapi.yaml は `issue` を
    /// required とし、`backlog-js` も非optionalと宣言しているが、どちらも誤り。
    /// これを信じていたら過半数のプルリクエストで一覧全体が失敗していた。
    #[test]
    fn deserializes_pull_request_without_issue() {
        let pr: PullRequest = serde_json::from_str(
            r#"{ "id": 10, "projectId": 2, "repositoryId": 3, "number": 7,
                 "summary": "設定を見直す", "description": "本文",
                 "base": "master", "branch": "topic/fix",
                 "status": { "id": 1, "name": "Open" },
                 "assignee": null, "issue": null,
                 "baseCommit": null, "branchCommit": null, "mergeCommit": null,
                 "closeAt": null, "mergeAt": null,
                 "createdUser": { "id": 1, "userId": "admin", "name": "admin", "roleType": 1 },
                 "created": "2013-05-30T09:11:36Z",
                 "updatedUser": { "id": 1, "userId": "admin", "name": "admin", "roleType": 1 },
                 "updated": "2013-05-30T09:11:36Z",
                 "attachments": [], "stars": [] }"#,
        )
        .unwrap();
        assert_eq!(pr.number, 7);
        assert!(pr.issue.is_none());
        assert!(pr.assignee.is_none());
        assert!(pr.merge_at.is_none());
        assert_eq!(pr.status.unwrap().name.as_deref(), Some("Open"));
        assert_eq!(pr.base.as_deref(), Some("master"));
        assert_eq!(pr.branch.as_deref(), Some("topic/fix"));
    }

    /// `status` が `{id, name}` だけでも受けられること。
    ///
    /// **既存の `super::Status` を流用したらここで失敗する。** あちらは
    /// `projectId` / `color` / `displayOrder` を必須にしているため、
    /// 実データの形(この形)が通らない。型を分けた根拠をこのテストで固定する。
    #[test]
    fn pull_request_status_needs_only_id_and_name() {
        let s: PullRequestStatus =
            serde_json::from_str(r#"{ "id": 3, "name": "Merged" }"#).unwrap();
        assert_eq!(s.id, 3);
        assert_eq!(s.name.as_deref(), Some("Merged"));

        // 参考: 既存の Status は同じJSONでは通らない(必須フィールド不足)
        assert!(
            serde_json::from_str::<super::super::Status>(r#"{ "id": 3, "name": "Merged" }"#)
                .is_err()
        );
    }

    /// 関連課題を持つプルリクエストで、既存の `Issue` に埋め込めること。
    ///
    /// 実データで49件を確認し、いずれも `Issue` の必須フィールドを持っていた。
    #[test]
    fn deserializes_pull_request_with_issue() {
        let pr: PullRequest = serde_json::from_str(
            r##"{ "id": 11, "projectId": 2, "repositoryId": 3, "number": 8,
                 "status": { "id": 3, "name": "Merged" },
                 "issue": { "id": 4531, "projectId": 2, "issueKey": "PROJ-17", "keyId": 17,
                            "issueType": { "id": 7, "projectId": 2, "name": "Bug",
                                           "color": "#990000", "displayOrder": 0 },
                            "summary": "課題の件名",
                            "status": { "id": 1, "projectId": 2, "name": "未対応",
                                        "color": "#ed8077", "displayOrder": 1000 },
                            "createdUser": { "id": 1, "userId": "admin",
                                             "name": "admin", "roleType": 1 },
                            "created": "2013-04-23T07:38:59Z",
                            "externalFileLinks": [] },
                 "mergeAt": "2013-06-01T00:00:00Z",
                 "attachments": [], "stars": [] }"##,
        )
        .unwrap();
        assert_eq!(pr.issue.unwrap().issue_key, "PROJ-17");
        assert!(pr.merge_at.is_some());
    }

    /// ファイル指定コメントと通常コメントの両方を受けられること。
    ///
    /// `filePath` / `position` / `oldBlobId` / `newBlobId` は openapi.yaml には
    /// あるが `backlog-js` には無い。実APIは返すので、`backlog-js` だけを見て
    /// モデルを書くとファイル指定コメントの情報を落とす。
    #[test]
    fn deserializes_comments_with_and_without_file_path() {
        let inline: PullRequestComment = serde_json::from_str(
            r#"{ "id": 1, "content": "この行は不要では",
                 "changeLog": [], "filePath": "src/main.rs", "position": 42,
                 "oldBlobId": "abc123", "newBlobId": "def456",
                 "createdUser": { "id": 1, "userId": "admin", "name": "admin", "roleType": 1 },
                 "created": "2013-05-30T09:11:36Z", "updated": "2013-05-30T09:11:36Z",
                 "stars": [], "notifications": [] }"#,
        )
        .unwrap();
        assert_eq!(inline.file_path.as_deref(), Some("src/main.rs"));
        assert_eq!(inline.position, Some(42));
        assert_eq!(inline.old_blob_id.as_deref(), Some("abc123"));

        let plain: PullRequestComment = serde_json::from_str(
            r#"{ "id": 2, "content": "全体としては良さそう",
                 "changeLog": [{ "field": "status" }],
                 "filePath": null, "position": null,
                 "oldBlobId": null, "newBlobId": null,
                 "createdUser": { "id": 1, "userId": "admin", "name": "admin", "roleType": 1 },
                 "created": "2013-05-30T09:11:36Z", "updated": "2013-05-30T09:11:36Z",
                 "stars": [], "notifications": [] }"#,
        )
        .unwrap();
        assert!(plain.file_path.is_none());
        assert!(plain.position.is_none());
        assert_eq!(plain.change_log.len(), 1);
    }

    /// 未型付けの配列が `null` でも欠落でも空になること。
    ///
    /// `attachments` / `stars` は実データで常に空配列だったため中身の形状が未観測。
    /// `serde_json::Value` のまま持つ判断を、null耐性とあわせて固定する。
    #[test]
    fn untyped_arrays_tolerate_null_and_absence() {
        let with_nulls: PullRequest = serde_json::from_str(
            r#"{ "id": 12, "projectId": 2, "repositoryId": 3, "number": 9,
                 "attachments": null, "stars": null }"#,
        )
        .unwrap();
        assert!(with_nulls.attachments.is_empty());
        assert!(with_nulls.stars.is_empty());

        let absent: PullRequest = serde_json::from_str(
            r#"{ "id": 13, "projectId": 2, "repositoryId": 3, "number": 10 }"#,
        )
        .unwrap();
        assert!(absent.attachments.is_empty());
        assert!(absent.stars.is_empty());
    }

    /// `PullRequest.attachments` / `stars` を型付けしない判断を固定する。
    ///
    /// 実データ100件はすべて空配列で、要素の形が未観測のため
    /// `serde_json::Value` のまま持つと決めた。既存の `Attachment`
    /// (`id`/`name`/`size` が必須)や `Star`
    /// (`id`/`url`/`presenter`/`created` が必須)に置き換えると、
    /// 想定外の形の要素が1つ来ただけで一覧全体のデシリアライズが失敗する。
    /// このテストは、想定外の形の要素があっても壊れず要素数を保つことを確認し、
    /// 誰かがこれらのフィールドを型付けし直すリファクタリングをした場合に
    /// 検知できるようにする。
    #[test]
    fn pull_request_untyped_arrays_survive_unexpected_element_shape() {
        let pr: PullRequest = serde_json::from_str(
            r#"{ "id": 14, "projectId": 2, "repositoryId": 3, "number": 11,
                 "attachments": [{ "weird": 1 }], "stars": [{ "weird": 2 }] }"#,
        )
        .unwrap();
        assert_eq!(pr.attachments.len(), 1);
        assert_eq!(pr.stars.len(), 1);
    }

    /// `PullRequestComment.stars` を型付けしない判断を固定する。
    ///
    /// 上と同じ理由(要素の形が未観測)で `serde_json::Value` のまま持っている。
    /// `Star` に置き換えると `id`/`url`/`presenter`/`created` の必須違反で
    /// 一覧全体が失敗するため、それを検知するテスト。
    #[test]
    fn pull_request_comment_stars_survive_unexpected_element_shape() {
        let comment: PullRequestComment =
            serde_json::from_str(r#"{ "id": 3, "stars": [{ "weird": 3 }] }"#).unwrap();
        assert_eq!(comment.stars.len(), 1);
    }
}
