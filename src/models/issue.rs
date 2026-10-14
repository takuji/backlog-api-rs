use chrono::{DateTime, Utc};
use serde::Deserialize;

use super::de::null_as_empty_vec;
use super::{Category, IssueType, Priority, Resolution, Status, User, Version};

/// Backlogの課題。
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Issue {
    pub id: u64,
    pub project_id: u64,
    pub issue_key: String,
    pub key_id: u64,
    pub issue_type: IssueType,
    pub summary: String,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub resolution: Option<Resolution>,
    #[serde(default)]
    pub priority: Option<Priority>,
    pub status: Status,
    #[serde(default)]
    pub assignee: Option<User>,
    #[serde(default, deserialize_with = "null_as_empty_vec")]
    pub category: Vec<Category>,
    #[serde(default, deserialize_with = "null_as_empty_vec")]
    pub versions: Vec<Version>,
    #[serde(default, deserialize_with = "null_as_empty_vec")]
    pub milestone: Vec<Version>,
    #[serde(default)]
    pub start_date: Option<DateTime<Utc>>,
    #[serde(default)]
    pub due_date: Option<DateTime<Utc>>,
    #[serde(default)]
    pub estimated_hours: Option<f64>,
    #[serde(default)]
    pub actual_hours: Option<f64>,
    #[serde(default)]
    pub parent_issue_id: Option<u64>,
    pub created_user: User,
    pub created: DateTime<Utc>,
    #[serde(default)]
    pub updated_user: Option<User>,
    #[serde(default)]
    pub updated: Option<DateTime<Utc>>,
    /// カスタム属性。マイルストーン1では未型付け。
    #[serde(default, deserialize_with = "null_as_empty_vec")]
    pub custom_fields: Vec<serde_json::Value>,
    #[serde(default, deserialize_with = "null_as_empty_vec")]
    pub attachments: Vec<Attachment>,
    #[serde(default, deserialize_with = "null_as_empty_vec")]
    pub shared_files: Vec<SharedFile>,
    #[serde(default, deserialize_with = "null_as_empty_vec")]
    pub stars: Vec<Star>,
}

/// 課題のコメント。状態変更のみのコメントでは `content` がnullになる。
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Comment {
    pub id: u64,
    pub project_id: u64,
    pub issue_id: u64,
    #[serde(default)]
    pub content: Option<String>,
    /// 変更履歴。マイルストーン1では未型付け。
    #[serde(default, deserialize_with = "null_as_empty_vec")]
    pub change_log: Vec<serde_json::Value>,
    pub created_user: User,
    pub created: DateTime<Utc>,
    #[serde(default)]
    pub updated: Option<DateTime<Utc>>,
    #[serde(default, deserialize_with = "null_as_empty_vec")]
    pub stars: Vec<Star>,
    /// 通知先。マイルストーン1では未型付け。
    #[serde(default, deserialize_with = "null_as_empty_vec")]
    pub notifications: Vec<serde_json::Value>,
}

/// 添付ファイル。
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Attachment {
    pub id: u64,
    pub name: String,
    pub size: u64,
    #[serde(default)]
    pub created_user: Option<User>,
    #[serde(default)]
    pub created: Option<DateTime<Utc>>,
}

/// 共有ファイル。
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SharedFile {
    pub id: u64,
    pub project_id: u64,
    #[serde(rename = "type")]
    pub file_type: String,
    pub dir: String,
    pub name: String,
    #[serde(default)]
    pub size: Option<u64>,
    pub created_user: User,
    pub created: DateTime<Utc>,
    #[serde(default)]
    pub updated_user: Option<User>,
    #[serde(default)]
    pub updated: Option<DateTime<Utc>>,
}

/// スター。
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Star {
    pub id: u64,
    #[serde(default)]
    pub comment: Option<String>,
    pub url: String,
    #[serde(default)]
    pub title: Option<String>,
    pub presenter: User,
    pub created: DateTime<Utc>,
}

#[cfg(test)]
mod tests {
    use super::*;

    // 実APIのレスポンス例に近い最小構成のフィクスチャ
    fn issue_json() -> &'static str {
        r##"{
            "id": 1, "projectId": 1, "issueKey": "BLG-1", "keyId": 1,
            "issueType": { "id": 2, "projectId": 1, "name": "タスク", "color": "#7ea800", "displayOrder": 0 },
            "summary": "first issue",
            "description": "",
            "resolution": null,
            "priority": { "id": 3, "name": "中" },
            "status": { "id": 1, "projectId": 1, "name": "未対応", "color": "#ed8077", "displayOrder": 1000 },
            "assignee": { "id": 2, "userId": "eguchi", "name": "eguchi", "roleType": 2, "lang": null, "mailAddress": "eguchi@nulab.example" },
            "category": [],
            "versions": [],
            "milestone": [
                { "id": 30, "projectId": 1, "name": "wait for release", "description": "", "startDate": null, "releaseDueDate": null, "archived": false, "displayOrder": 0 }
            ],
            "startDate": null,
            "dueDate": null,
            "estimatedHours": null,
            "actualHours": null,
            "parentIssueId": null,
            "createdUser": { "id": 1, "userId": "admin", "name": "admin", "roleType": 1, "lang": "ja", "mailAddress": "eguchi@nulab.example" },
            "created": "2012-07-23T06:10:15Z",
            "updatedUser": { "id": 1, "userId": "admin", "name": "admin", "roleType": 1, "lang": "ja", "mailAddress": "eguchi@nulab.example" },
            "updated": "2013-02-07T08:09:49Z",
            "customFields": [],
            "attachments": [{ "id": 1, "name": "IMGP0088.JPG", "size": 85079 }],
            "sharedFiles": [],
            "stars": []
        }"##
    }

    #[test]
    fn deserializes_issue() {
        let issue: Issue = serde_json::from_str(issue_json()).unwrap();
        assert_eq!(issue.issue_key, "BLG-1");
        assert_eq!(issue.status.name, "未対応");
        assert_eq!(issue.priority.as_ref().unwrap().id, 3);
        assert!(issue.resolution.is_none());
        assert_eq!(issue.milestone.len(), 1);
        assert_eq!(issue.attachments[0].size, 85079);
        assert_eq!(issue.assignee.as_ref().unwrap().name, "eguchi");
    }

    #[test]
    fn deserializes_comment() {
        // 公式ドキュメント(get-comment-list)の応答例は changeLog が null。
        // `#[serde(default)]` だけでは失敗するため、ここを null にしておくことで
        // null_as_empty_vec が効いていることを常に検証する。
        let json = r#"{
            "id": 6586, "projectId": 1, "issueId": 1102,
            "content": "test",
            "changeLog": null,
            "createdUser": { "id": 1, "userId": "admin", "name": "admin", "roleType": 1 },
            "created": "2013-08-05T06:15:06Z",
            "updated": "2013-08-05T06:15:06Z",
            "stars": [],
            "notifications": []
        }"#;
        let comment: Comment = serde_json::from_str(json).unwrap();
        assert_eq!(comment.content.as_deref(), Some("test"));
        assert_eq!(comment.issue_id, 1102);
    }

    #[test]
    fn deserializes_status_change_comment_with_null_content() {
        let json = r#"{
            "id": 6587, "projectId": 1, "issueId": 1102,
            "content": null,
            "changeLog": [{ "field": "status", "newValue": "完了", "originalValue": "未対応" }],
            "createdUser": { "id": 1, "userId": "admin", "name": "admin", "roleType": 1 },
            "created": "2013-08-05T06:15:06Z",
            "updated": "2013-08-05T06:15:06Z",
            "stars": [],
            "notifications": []
        }"#;
        let comment: Comment = serde_json::from_str(json).unwrap();
        assert_eq!(comment.content, None);
        assert_eq!(comment.change_log.len(), 1);
    }
}
