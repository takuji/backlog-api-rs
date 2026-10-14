use chrono::{DateTime, Utc};
use serde::Deserialize;

/// Backlogのユーザー。
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct User {
    pub id: u64,
    #[serde(default)]
    pub user_id: Option<String>,
    pub name: String,
    /// 権限種別。1=管理者など。<https://developer.nulab.com/docs/backlog/api/2/get-user-list/>
    pub role_type: u8,
    #[serde(default)]
    pub lang: Option<String>,
    #[serde(default)]
    pub mail_address: Option<String>,
    #[serde(default)]
    pub last_login_time: Option<DateTime<Utc>>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn deserializes_user() {
        let json = r#"{
            "id": 1,
            "userId": "admin",
            "name": "admin",
            "roleType": 1,
            "lang": "ja",
            "mailAddress": "hoge@nulab.example",
            "lastLoginTime": "2022-09-01T06:35:39Z"
        }"#;
        let user: User = serde_json::from_str(json).unwrap();
        assert_eq!(user.id, 1);
        assert_eq!(user.user_id.as_deref(), Some("admin"));
        assert_eq!(user.role_type, 1);
    }

    #[test]
    fn deserializes_user_with_nulls_and_missing_fields() {
        let json = r#"{ "id": 2, "name": "bot", "roleType": 2, "lang": null }"#;
        let user: User = serde_json::from_str(json).unwrap();
        assert_eq!(user.user_id, None);
        assert_eq!(user.lang, None);
        assert_eq!(user.last_login_time, None);
    }
}
