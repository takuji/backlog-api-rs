use chrono::{DateTime, Utc};
use serde::Deserialize;

use super::User;
use super::de::{de_scalar_as_string, null_as_empty_vec};

/// Backlog のドキュメント。
///
/// 本文は `json`(リッチテキストの構造)と `plain`(プレーンテキスト)の両方で返る。
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Document {
    /// ドキュメントID(文字列。課題やWikiと異なり数値ではない)
    pub id: String,
    pub project_id: u64,
    pub title: String,
    /// リッチテキスト本文の構造。実APIはJSONエンコードされた文字列を返すが、
    /// `serde_json::Value` は文字列でもオブジェクトでも受けられるためこのままでよい。
    /// 用途がなければ `plain` を使う。
    #[serde(default)]
    pub json: Option<serde_json::Value>,
    /// プレーンテキストの本文。
    #[serde(default)]
    pub plain: Option<String>,
    /// タイトル前に表示される絵文字。
    #[serde(default)]
    pub emoji: Option<String>,
    /// 実APIは数値で返すが、openapi.yamlの記載(文字列)でも受け取れるようにしている。
    #[serde(deserialize_with = "de_scalar_as_string")]
    pub status_id: String,
    #[serde(default, deserialize_with = "null_as_empty_vec")]
    pub attachments: Vec<DocumentAttachment>,
    #[serde(default, deserialize_with = "null_as_empty_vec")]
    pub tags: Vec<DocumentTag>,
    pub created_user: User,
    pub created: DateTime<Utc>,
    #[serde(default)]
    pub updated_user: Option<User>,
    #[serde(default)]
    pub updated: Option<DateTime<Utc>>,
}

/// ドキュメントのタグ。
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DocumentTag {
    pub id: u64,
    pub name: String,
}

/// ドキュメントの添付ファイル。
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DocumentAttachment {
    /// 添付ファイルID。実APIは数値で返すが文字列でも受け取れるようにしている。
    #[serde(deserialize_with = "de_scalar_as_string")]
    pub id: String,
    pub name: String,
    pub size: u64,
    #[serde(default)]
    pub created_user: Option<User>,
    #[serde(default)]
    pub created: Option<DateTime<Utc>>,
}

/// ドキュメントのコメント。
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DocumentComment {
    /// コメントID(文字列)
    pub id: String,
    pub document_id: String,
    /// 書式付きのコメント本文。
    #[serde(default)]
    pub content: Option<String>,
    /// プレーンテキストのコメント本文。
    #[serde(default)]
    pub plain: Option<String>,
    #[serde(default)]
    pub created_user: Option<User>,
    pub created: DateTime<Utc>,
    #[serde(default)]
    pub updated: Option<DateTime<Utc>>,
}

/// `GET /api/v2/documents/tree` の応答全体。
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DocumentTree {
    pub project_id: u64,
    /// 通常のドキュメントツリー。
    pub active_tree: DocumentTreeNode,
    /// ゴミ箱のツリー。
    #[serde(default)]
    pub trash_tree: Option<DocumentTreeNode>,
}

/// ドキュメントツリーのノード。`children` で再帰する。
///
/// ルートノード(`id` が `"Active"` / `"Trash"`)は `name` を持たない。
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DocumentTreeNode {
    pub id: String,
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default)]
    pub emoji: Option<String>,
    #[serde(default, deserialize_with = "null_as_empty_vec")]
    pub children: Vec<DocumentTreeNode>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn deserializes_document_with_content_and_tags() {
        let json = r##"{
            "id": "doc-abc123",
            "projectId": 7,
            "title": "設計メモ",
            "json": "{}",
            "plain": "本文のプレーンテキスト",
            "emoji": "📝",
            "statusId": 1,
            "attachments": [{
                "id": 22067, "name": "spec.pdf", "size": 2048,
                "createdUser": { "id": 1, "userId": "admin", "name": "admin", "roleType": 1 },
                "created": "2026-07-01T06:10:15Z"
            }],
            "tags": [{ "id": 3, "name": "設計" }],
            "createdUser": { "id": 1, "userId": "admin", "name": "admin", "roleType": 1 },
            "created": "2026-07-01T06:10:15Z",
            "updatedUser": { "id": 1, "userId": "admin", "name": "admin", "roleType": 1 },
            "updated": "2026-07-20T08:09:49Z"
        }"##;
        let doc: Document = serde_json::from_str(json).unwrap();
        assert_eq!(doc.id, "doc-abc123");
        assert_eq!(doc.title, "設計メモ");
        assert_eq!(doc.plain.as_deref(), Some("本文のプレーンテキスト"));
        assert!(doc.json.is_some());
        assert_eq!(doc.emoji.as_deref(), Some("📝"));
        assert_eq!(doc.status_id, "1");
        assert_eq!(doc.attachments[0].id, "22067");
        assert_eq!(doc.attachments[0].size, 2048);
        assert_eq!(
            doc.attachments[0].created_user.as_ref().unwrap().name,
            "admin"
        );
        assert!(doc.attachments[0].created.is_some());
        assert_eq!(doc.tags[0].name, "設計");
    }

    #[test]
    fn deserializes_document_with_null_content_and_missing_optionals() {
        let json = r##"{
            "id": "doc-empty",
            "projectId": 7,
            "title": "空",
            "json": null,
            "plain": null,
            "emoji": null,
            "statusId": 1,
            "createdUser": { "id": 1, "userId": "admin", "name": "admin", "roleType": 1 },
            "created": "2026-07-01T06:10:15Z"
        }"##;
        let doc: Document = serde_json::from_str(json).unwrap();
        assert_eq!(doc.json, None);
        assert_eq!(doc.plain, None);
        assert_eq!(doc.emoji, None);
        assert!(doc.attachments.is_empty());
        assert!(doc.tags.is_empty());
        assert!(doc.updated_user.is_none());
        assert!(doc.updated.is_none());
    }

    #[test]
    fn deserializes_document_comment() {
        let json = r##"{
            "id": "cmt-1",
            "documentId": "doc-abc123",
            "statusId": 1,
            "content": "**確認しました**",
            "plain": "確認しました",
            "commentType": 1,
            "createdUserId": 2,
            "created": "2026-07-21T01:00:00Z",
            "updatedUserId": null,
            "updated": "2026-07-21T01:00:00Z",
            "createdUser": { "id": 2, "userId": "eguchi", "name": "eguchi", "roleType": 2 }
        }"##;
        let comment: DocumentComment = serde_json::from_str(json).unwrap();
        assert_eq!(comment.id, "cmt-1");
        assert_eq!(comment.document_id, "doc-abc123");
        assert_eq!(comment.plain.as_deref(), Some("確認しました"));
        assert_eq!(comment.created_user.as_ref().unwrap().name, "eguchi");
    }

    #[test]
    fn accepts_status_id_and_attachment_id_as_number_or_string() {
        // 実API: 数値
        let numeric = r##"{
            "id": "doc-1", "projectId": 7, "title": "t", "json": "{}", "plain": "p",
            "emoji": null, "statusId": 1,
            "attachments": [{ "id": 22067, "name": "a.png", "size": 10 }],
            "tags": [],
            "createdUser": { "id": 1, "userId": "admin", "name": "admin", "roleType": 1 },
            "created": "2026-07-01T06:10:15Z"
        }"##;
        let doc: Document = serde_json::from_str(numeric).unwrap();
        assert_eq!(doc.status_id, "1");
        assert_eq!(doc.attachments[0].id, "22067");

        // openapi.yaml の記載: 文字列
        let stringy = r##"{
            "id": "doc-1", "projectId": 7, "title": "t", "json": "{}", "plain": "p",
            "emoji": null, "statusId": "active",
            "attachments": [{ "id": "att-1", "name": "a.png", "size": 10 }],
            "tags": [],
            "createdUser": { "id": 1, "userId": "admin", "name": "admin", "roleType": 1 },
            "created": "2026-07-01T06:10:15Z"
        }"##;
        let doc: Document = serde_json::from_str(stringy).unwrap();
        assert_eq!(doc.status_id, "active");
        assert_eq!(doc.attachments[0].id, "att-1");
    }

    #[test]
    fn deserializes_document_tree_from_official_example() {
        let json = r##"{
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
        }"##;
        let tree: DocumentTree = serde_json::from_str(json).unwrap();
        assert_eq!(tree.project_id, 1);
        // ルートは name を持たない
        assert_eq!(tree.active_tree.id, "Active");
        assert_eq!(tree.active_tree.name, None);
        let top = &tree.active_tree.children[0];
        assert_eq!(top.name.as_deref(), Some("local test"));
        assert_eq!(
            top.children[0].children[0].name.as_deref(),
            Some("aaatest234")
        );
        assert!(tree.trash_tree.is_some());
        assert!(tree.trash_tree.as_ref().unwrap().children.is_empty());
    }

    #[test]
    fn deserializes_document_tree_without_trash_tree() {
        // 実APIのレスポンスには trashTree キー自体が存在しないことがある。
        let json = r##"{
            "projectId": 7,
            "activeTree": {
                "id": "Active",
                "children": [
                    {
                        "id": "019f3af2d2c8713cb91f488cede6fd0f",
                        "name": "議事録",
                        "emoji": "",
                        "children": []
                    },
                    {
                        "id": "019e44979f8872468dac11e53b5c2c9e",
                        "name": "設計ノート",
                        "emoji": "",
                        "children": [
                            {
                                "id": "019eee967ed27101998748371c2fa882",
                                "name": "詳細設計",
                                "emoji": "",
                                "children": []
                            }
                        ]
                    }
                ]
            }
        }"##;
        let tree: DocumentTree = serde_json::from_str(json).unwrap();
        assert_eq!(tree.project_id, 7);
        assert!(tree.trash_tree.is_none());
        assert_eq!(tree.active_tree.id, "Active");
        assert_eq!(tree.active_tree.children.len(), 2);
        assert_eq!(tree.active_tree.children[0].name.as_deref(), Some("議事録"));
        assert_eq!(
            tree.active_tree.children[1].name.as_deref(),
            Some("設計ノート")
        );
        // ネストした子ノードも解釈できる
        assert_eq!(
            tree.active_tree.children[1].children[0].name.as_deref(),
            Some("詳細設計")
        );
    }
}
