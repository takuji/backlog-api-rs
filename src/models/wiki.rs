use chrono::{DateTime, Utc};
use serde::Deserialize;

use super::de::null_as_empty_vec;
use super::{SharedFile, Star, User};

/// Backlog の Wiki ページ。
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Wiki {
    pub id: u64,
    pub project_id: u64,
    pub name: String,
    #[serde(default)]
    pub content: Option<String>,
    #[serde(default, deserialize_with = "null_as_empty_vec")]
    pub tags: Vec<WikiTag>,
    #[serde(default, deserialize_with = "null_as_empty_vec")]
    pub attachments: Vec<WikiAttachment>,
    #[serde(default, deserialize_with = "null_as_empty_vec")]
    pub shared_files: Vec<SharedFile>,
    #[serde(default, deserialize_with = "null_as_empty_vec")]
    pub stars: Vec<Star>,
    pub created_user: User,
    pub created: DateTime<Utc>,
    #[serde(default)]
    pub updated_user: Option<User>,
    #[serde(default)]
    pub updated: Option<DateTime<Utc>>,
}

/// Wiki のタグ。
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WikiTag {
    pub id: u64,
    pub name: String,
}

/// Wiki の添付ファイル。
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WikiAttachment {
    pub id: u64,
    pub name: String,
    pub size: u64,
    #[serde(default)]
    pub created_user: Option<User>,
    #[serde(default)]
    pub created: Option<DateTime<Utc>>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn deserializes_wiki_with_content_and_tags() {
        let json = "{
            \"id\": 1, \"projectId\": 7, \"name\": \"Home\",
            \"content\": \"# ようこそ\\n\\n本文です\",
            \"tags\": [{ \"id\": 12, \"name\": \"議事録\" }],
            \"attachments\": [{ \"id\": 5, \"name\": \"spec.pdf\", \"size\": 1024 }],
            \"sharedFiles\": [],
            \"stars\": [],
            \"createdUser\": { \"id\": 1, \"userId\": \"admin\", \"name\": \"admin\", \"roleType\": 1 },
            \"created\": \"2026-07-01T06:10:15Z\",
            \"updatedUser\": { \"id\": 1, \"userId\": \"admin\", \"name\": \"admin\", \"roleType\": 1 },
            \"updated\": \"2026-07-20T08:09:49Z\"
        }";
        let wiki: Wiki = serde_json::from_str(json).unwrap();
        assert_eq!(wiki.id, 1);
        assert_eq!(wiki.name, "Home");
        assert!(wiki.content.as_deref().unwrap().contains("本文です"));
        assert_eq!(wiki.tags[0].name, "議事録");
        assert_eq!(wiki.attachments[0].size, 1024);
    }

    #[test]
    fn deserializes_wiki_with_null_content_and_missing_optionals() {
        let json = "{
            \"id\": 2, \"projectId\": 7, \"name\": \"空ページ\",
            \"content\": null, \"tags\": [], \"attachments\": [],
            \"sharedFiles\": [], \"stars\": [],
            \"createdUser\": { \"id\": 1, \"userId\": \"admin\", \"name\": \"admin\", \"roleType\": 1 },
            \"created\": \"2026-07-01T06:10:15Z\"
        }";
        let wiki: Wiki = serde_json::from_str(json).unwrap();
        assert_eq!(wiki.content, None);
        assert!(wiki.tags.is_empty());
        assert!(wiki.updated_user.is_none());
        assert!(wiki.updated.is_none());
    }

    #[test]
    fn deserializes_wiki_tag() {
        let tag: WikiTag = serde_json::from_str("{ \"id\": 3, \"name\": \"設計\" }").unwrap();
        assert_eq!(tag.id, 3);
        assert_eq!(tag.name, "設計");
    }
}
