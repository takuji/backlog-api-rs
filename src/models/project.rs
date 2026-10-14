use chrono::{DateTime, Utc};
use serde::Deserialize;

/// Backlogのプロジェクト。
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Project {
    pub id: u64,
    pub project_key: String,
    pub name: String,
    pub chart_enabled: bool,
    pub subtasking_enabled: bool,
    pub project_leader_can_edit_project_leader: bool,
    pub use_wiki: bool,
    pub use_file_sharing: bool,
    pub use_wiki_tree_view: bool,
    pub use_original_image_size_at_wiki: bool,
    /// `"backlog"` または `"markdown"`。
    pub text_formatting_rule: String,
    pub archived: bool,
    pub display_order: i64,
    pub use_dev_attributes: bool,
}

/// 課題種別。
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct IssueType {
    pub id: u64,
    pub project_id: u64,
    pub name: String,
    pub color: String,
    pub display_order: i64,
}

/// 課題の状態。
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Status {
    pub id: u64,
    pub project_id: u64,
    pub name: String,
    pub color: String,
    pub display_order: i64,
}

/// 優先度。
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Priority {
    pub id: u64,
    pub name: String,
}

/// 完了理由。
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Resolution {
    pub id: u64,
    pub name: String,
}

/// カテゴリー。
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Category {
    pub id: u64,
    pub name: String,
    pub display_order: i64,
}

/// 発生バージョン・マイルストーン。
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Version {
    pub id: u64,
    pub project_id: u64,
    pub name: String,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub start_date: Option<DateTime<Utc>>,
    #[serde(default)]
    pub release_due_date: Option<DateTime<Utc>>,
    pub archived: bool,
    pub display_order: i64,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn deserializes_project() {
        let json = r#"{
            "id": 7, "projectKey": "TEST1", "name": "TEST1",
            "chartEnabled": true, "subtaskingEnabled": false,
            "projectLeaderCanEditProjectLeader": false,
            "useWiki": true, "useFileSharing": true, "useWikiTreeView": true,
            "useOriginalImageSizeAtWiki": false,
            "textFormattingRule": "markdown", "archived": false,
            "displayOrder": 2147483646, "useDevAttributes": true
        }"#;
        let project: Project = serde_json::from_str(json).unwrap();
        assert_eq!(project.project_key, "TEST1");
        assert_eq!(project.text_formatting_rule, "markdown");
    }

    #[test]
    fn deserializes_issue_type_and_status() {
        let issue_type: IssueType = serde_json::from_str(
            r##"{ "id": 8, "projectId": 7, "name": "バグ", "color": "#990000", "displayOrder": 0 }"##,
        )
        .unwrap();
        assert_eq!(issue_type.name, "バグ");

        let status: Status = serde_json::from_str(
            r##"{ "id": 1, "projectId": 7, "name": "未対応", "color": "#ed8077", "displayOrder": 1000 }"##,
        )
        .unwrap();
        assert_eq!(status.name, "未対応");
    }

    #[test]
    fn deserializes_priority_resolution_category_version() {
        let priority: Priority = serde_json::from_str(r#"{ "id": 3, "name": "中" }"#).unwrap();
        assert_eq!(priority.id, 3);

        let resolution: Resolution =
            serde_json::from_str(r#"{ "id": 0, "name": "対応済み" }"#).unwrap();
        assert_eq!(resolution.id, 0);

        let category: Category =
            serde_json::from_str(r#"{ "id": 12, "name": "開発", "displayOrder": 0 }"#).unwrap();
        assert_eq!(category.name, "開発");

        let version: Version = serde_json::from_str(
            r#"{
                "id": 3, "projectId": 1, "name": "いつか", "description": null,
                "startDate": null, "releaseDueDate": "2024-06-30T00:00:00Z",
                "archived": false, "displayOrder": 0
            }"#,
        )
        .unwrap();
        assert_eq!(version.start_date, None);
        assert!(version.release_due_date.is_some());
    }
}
