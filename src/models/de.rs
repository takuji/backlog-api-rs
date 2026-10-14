//! 複数のモデルで共有する寛容なデシリアライザ。
//!
//! Backlog API は openapi.yaml の型と食い違う値を返すことがある。
//! ここには**複数のモデルにまたがる**変換を置く(1つのモデルにしか
//! 必要ないものはそのモデルのファイルに置く)。

use serde::Deserialize;

/// 数値・文字列のどちらで返っても `String` として受け取る。
///
/// Backlog の内部OpenAPI仕様と公式ドキュメントでこれらのフィールドの型が
/// 食い違っていたため、どちらでも壊れないようにしている。同じ論理的な概念
/// (ID)でも、リソースによって数値で返ることも文字列で返ることもある
/// (例: `Comment.id` は数値、`DocumentComment.id` は文字列)ため、
/// 複数のモデルで必要になりここに置く。
pub(crate) fn de_scalar_as_string<'de, D>(deserializer: D) -> Result<String, D::Error>
where
    D: serde::Deserializer<'de>,
{
    use serde::Deserialize as _;

    #[derive(Deserialize)]
    #[serde(untagged)]
    enum Scalar {
        Int(i64),
        Str(String),
    }

    Ok(match Scalar::deserialize(deserializer)? {
        Scalar::Int(n) => n.to_string(),
        Scalar::Str(s) => s,
    })
}

/// [`de_scalar_as_string`] の `Option` 版。
///
/// 数値・文字列に加えて `null` および欠落フィールドも受け、`Option<String>` を返す。
/// `#[serde(default, deserialize_with = "de_optional_scalar_as_string")]` の形で使う
/// (`default` も併記すること。フィールド自体が欠けている場合はこちらが効く)。
pub(crate) fn de_optional_scalar_as_string<'de, D>(
    deserializer: D,
) -> Result<Option<String>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    use serde::Deserialize as _;

    #[derive(Deserialize)]
    #[serde(untagged)]
    enum Scalar {
        Int(i64),
        Str(String),
    }

    Ok(
        Option::<Scalar>::deserialize(deserializer)?.map(|scalar| match scalar {
            Scalar::Int(n) => n.to_string(),
            Scalar::Str(s) => s,
        }),
    )
}

/// `null` を空の `Vec` として受ける。
///
/// `#[serde(default)]` はフィールドが**欠けている**場合にしか効かない。
/// 値が `null` だと `invalid type: null, expected a sequence` で失敗する。
/// Backlog API は空のコレクションを `null` で返すことがあり、公式ドキュメントの
/// 応答例でも `changeLog: null` を確認している。
///
/// `#[serde(default, deserialize_with = "null_as_empty_vec")]` の形で使う
/// (`default` も併記すること。フィールド自体が欠けている場合はこちらが効く)。
pub(crate) fn null_as_empty_vec<'de, D, T>(deserializer: D) -> Result<Vec<T>, D::Error>
where
    D: serde::Deserializer<'de>,
    T: Deserialize<'de>,
{
    Ok(Option::<Vec<T>>::deserialize(deserializer)?.unwrap_or_default())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Debug, Deserialize)]
    struct Holder {
        #[serde(default, deserialize_with = "null_as_empty_vec")]
        items: Vec<u32>,
    }

    /// `null` / 欠落 / 配列の3通りすべてを受けること。
    ///
    /// `null` が本題。`#[serde(default)]` だけでは型エラーになるため、
    /// このヘルパーが存在する。
    #[test]
    fn accepts_null_missing_and_array() {
        let from_null: Holder = serde_json::from_str(r#"{"items": null}"#).unwrap();
        assert_eq!(from_null.items, Vec::<u32>::new());

        let from_missing: Holder = serde_json::from_str(r#"{}"#).unwrap();
        assert_eq!(from_missing.items, Vec::<u32>::new());

        let from_array: Holder = serde_json::from_str(r#"{"items": [1, 2, 3]}"#).unwrap();
        assert_eq!(from_array.items, vec![1, 2, 3]);
    }

    /// 型が違う場合は従来どおりエラーにする(寛容にしすぎない)。
    #[test]
    fn rejects_wrong_type() {
        assert!(serde_json::from_str::<Holder>(r#"{"items": 5}"#).is_err());
        assert!(serde_json::from_str::<Holder>(r#"{"items": "x"}"#).is_err());
    }

    /// すべてのモデルの `Vec` フィールドが `null` を受けられること。
    ///
    /// 既存の fixture はすべて `[]` を手書きしていたため、実APIが `null` を
    /// 返した場合に一覧が丸ごと取れなくなる欠陥を検知できなかった。
    /// **`null_as_empty_vec` を使うフィールドは1つ残らずここに追加すること。**
    /// 件数を書くと増えるたびに陳腐化するので数は書かない。網羅を確認するには:
    ///
    /// ```sh
    /// grep -A1 'null_as_empty_vec' src/models/*.rs | grep 'pub .*: Vec<' | grep -v '///' | wc -l
    /// ```
    ///
    /// `grep -v '///'` を忘れると、この recipe 自身を引用しているこの doc
    /// comment の行(この行に `null_as_empty_vec` と `pub .*: Vec<` の両方の
    /// 文字列が含まれる)を1件多く数えてしまう。
    /// この数とこのテストが触るフィールド数が一致していること。
    #[test]
    fn every_model_accepts_null_for_its_vec_fields() {
        use crate::models::{
            Comment, Document, DocumentTreeNode, Issue, PullRequest, PullRequestComment, Wiki,
        };

        // Issue: category / versions / milestone / custom_fields /
        //        attachments / shared_files / stars
        let issue: Issue = serde_json::from_str(
            r##"{
                "id": 1, "projectId": 2, "issueKey": "T-1", "keyId": 1,
                "issueType": { "id": 7, "projectId": 2, "name": "Bug",
                               "color": "#990000", "displayOrder": 0 },
                "summary": "s",
                "status": { "id": 1, "projectId": 2, "name": "Open",
                            "color": "#ed8077", "displayOrder": 1000 },
                "createdUser": { "id": 1, "userId": "admin", "name": "admin", "roleType": 1 },
                "created": "2013-04-23T07:38:59Z",
                "category": null, "versions": null, "milestone": null,
                "customFields": null, "attachments": null, "sharedFiles": null, "stars": null
            }"##,
        )
        .unwrap();
        assert!(issue.category.is_empty());
        assert!(issue.versions.is_empty());
        assert!(issue.milestone.is_empty());
        assert!(issue.custom_fields.is_empty());
        assert!(issue.attachments.is_empty());
        assert!(issue.shared_files.is_empty());
        assert!(issue.stars.is_empty());

        // Comment: change_log / stars / notifications
        // changeLog が null なのは公式ドキュメントの応答例どおり。
        let comment: Comment = serde_json::from_str(
            r#"{
                "id": 1, "projectId": 2, "issueId": 3, "content": "c",
                "changeLog": null,
                "createdUser": { "id": 1, "userId": "admin", "name": "admin", "roleType": 1 },
                "created": "2013-08-05T06:15:06Z",
                "stars": null, "notifications": null
            }"#,
        )
        .unwrap();
        assert!(comment.change_log.is_empty());
        assert!(comment.stars.is_empty());
        assert!(comment.notifications.is_empty());

        // Wiki: tags / attachments / shared_files / stars
        let wiki: Wiki = serde_json::from_str(
            r#"{
                "id": 1, "projectId": 2, "name": "Home", "content": "c",
                "tags": null, "attachments": null, "sharedFiles": null, "stars": null,
                "createdUser": { "id": 1, "userId": "admin", "name": "admin", "roleType": 1 },
                "created": "2013-05-30T09:11:36Z",
                "updatedUser": { "id": 1, "userId": "admin", "name": "admin", "roleType": 1 },
                "updated": "2013-05-30T09:11:36Z"
            }"#,
        )
        .unwrap();
        assert!(wiki.tags.is_empty());
        assert!(wiki.attachments.is_empty());
        assert!(wiki.shared_files.is_empty());
        assert!(wiki.stars.is_empty());

        // Document: attachments / tags
        let document: Document = serde_json::from_str(
            r#"{
                "id": "abc", "projectId": 2, "title": "t", "plain": "p", "json": "{}",
                "statusId": 1, "attachments": null, "tags": null,
                "createdUser": { "id": 1, "userId": "admin", "name": "admin", "roleType": 1 },
                "created": "2024-01-01T00:00:00Z",
                "updatedUser": { "id": 1, "userId": "admin", "name": "admin", "roleType": 1 },
                "updated": "2024-01-01T00:00:00Z"
            }"#,
        )
        .unwrap();
        assert!(document.attachments.is_empty());
        assert!(document.tags.is_empty());

        // DocumentTreeNode: children
        let node: DocumentTreeNode =
            serde_json::from_str(r#"{ "id": "x", "name": "n", "children": null }"#).unwrap();
        assert!(node.children.is_empty());

        // PullRequest: attachments / stars
        let pull_request: PullRequest = serde_json::from_str(
            r#"{
                "id": 1, "projectId": 2, "repositoryId": 3, "number": 1,
                "attachments": null, "stars": null
            }"#,
        )
        .unwrap();
        assert!(pull_request.attachments.is_empty());
        assert!(pull_request.stars.is_empty());

        // PullRequestComment: change_log / stars / notifications
        let pull_request_comment: PullRequestComment = serde_json::from_str(
            r#"{
                "id": 1,
                "changeLog": null, "stars": null, "notifications": null
            }"#,
        )
        .unwrap();
        assert!(pull_request_comment.change_log.is_empty());
        assert!(pull_request_comment.stars.is_empty());
        assert!(pull_request_comment.notifications.is_empty());
    }
}
