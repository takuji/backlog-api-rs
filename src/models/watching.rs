use chrono::{DateTime, Utc};
use serde::Deserialize;

use super::Issue;

/// ウォッチしている課題。
///
/// `note` / `type` / `issue` / `last_content_updated` / `updated` の5つは
/// openapi.yaml では全て required とされているが、信用しない
/// (このプロジェクトは openapi.yaml の required を信じて壊れた実績がある)。
/// Tauriアプリも欠けても壊れないように扱っており、ここでも全て `Option` にする。
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Watching {
    pub id: u64,
    pub resource_already_read: bool,
    #[serde(default)]
    pub note: Option<String>,
    /// 定数 `"issue"`。
    #[serde(default)]
    pub r#type: Option<String>,
    #[serde(default)]
    pub issue: Option<Issue>,
    #[serde(default)]
    pub last_content_updated: Option<DateTime<Utc>>,
    pub created: DateTime<Utc>,
    #[serde(default)]
    pub updated: Option<DateTime<Utc>>,
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 公式ドキュメント(get-watching-list)の応答例をそのままデシリアライズできること。
    ///
    /// fixture は https://developer.nulab.com/docs/backlog/api/2/get-watching-list/
    /// の応答例を書き起こさずそのまま使っている。
    #[test]
    fn deserializes_the_official_response_example() {
        let list: Vec<Watching> = serde_json::from_str(OFFICIAL_EXAMPLE).unwrap();
        assert_eq!(list.len(), 1);
        let w = &list[0];
        assert_eq!(w.id, 1);
        assert!(w.resource_already_read);
        assert_eq!(
            w.note.as_deref(),
            Some("This is a note for the watching issue.")
        );
        assert_eq!(w.r#type.as_deref(), Some("issue"));
        assert_eq!(w.issue.as_ref().unwrap().issue_key, "TEST2-17");
        assert!(w.last_content_updated.is_some());
    }

    /// `note` / `last_content_updated` / `issue` が欠けても null でも通ること。
    ///
    /// openapi.yaml はこれらを required としているが、信用しない。
    #[test]
    fn deserializes_watching_without_optional_fields() {
        let w: Watching = serde_json::from_str(
            r#"{ "id": 2, "resourceAlreadyRead": false,
                 "created": "2013-10-31T06:58:59Z" }"#,
        )
        .unwrap();
        assert!(w.note.is_none());
        assert!(w.issue.is_none());
        assert!(w.last_content_updated.is_none());

        let w: Watching = serde_json::from_str(
            r#"{ "id": 3, "resourceAlreadyRead": false,
                 "note": null, "type": null, "issue": null,
                 "lastContentUpdated": null, "updated": null,
                 "created": "2013-10-31T06:58:59Z" }"#,
        )
        .unwrap();
        assert!(w.note.is_none());
        assert!(w.issue.is_none());
    }

    /// https://developer.nulab.com/docs/backlog/api/2/get-watching-list/ の応答例。
    const OFFICIAL_EXAMPLE: &str = r##"[
    {
        "id": 1,
        "resourceAlreadyRead": true,
        "note": "This is a note for the watching issue.",
        "type": "issue",
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
        "lastContentUpdated": "2013-10-31T06:58:59Z",
        "created": "2013-10-31T06:58:59Z",
        "updated": "2013-10-31T06:58:59Z"
    }
]"##;
}
