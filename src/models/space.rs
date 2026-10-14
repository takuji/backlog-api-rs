use chrono::{DateTime, Utc};
use serde::Deserialize;

/// Backlogのスペース。
///
/// 実APIで観測した1スペースでは9フィールドすべてが存在し `null` は無かったが、
/// 観測が1件しかないため、`space_key` と `name` 以外は `Option` にしておく。
/// 1件でも形の合わない応答が来れば呼び出しごと失敗するので、寛容側に倒す。
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Space {
    pub space_key: String,
    pub name: String,
    #[serde(default)]
    pub owner_id: Option<u64>,
    #[serde(default)]
    pub lang: Option<String>,
    #[serde(default)]
    pub timezone: Option<String>,
    #[serde(default)]
    pub report_send_time: Option<String>,
    #[serde(default)]
    pub text_formatting_rule: Option<String>,
    #[serde(default)]
    pub created: Option<DateTime<Utc>>,
    #[serde(default)]
    pub updated: Option<DateTime<Utc>>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;

    /// 実APIで観測した形をデシリアライズできること。
    #[test]
    fn deserializes_the_observed_shape() {
        let s: Space = serde_json::from_str(
            r#"{ "spaceKey": "EXAMPLE", "name": "サンプルスペース", "ownerId": 1,
                 "lang": "ja", "timezone": "Asia/Tokyo", "reportSendTime": "08:00:00",
                 "textFormattingRule": "backlog",
                 "created": "2013-05-30T09:11:36Z", "updated": "2013-06-15T10:20:45Z" }"#,
        )
        .unwrap();
        assert_eq!(s.space_key, "EXAMPLE");
        assert_eq!(s.name, "サンプルスペース");
        assert_eq!(s.owner_id, Some(1));
        assert_eq!(s.lang.as_deref(), Some("ja"));
        assert_eq!(s.timezone.as_deref(), Some("Asia/Tokyo"));
        assert_eq!(s.report_send_time.as_deref(), Some("08:00:00"));
        assert_eq!(s.text_formatting_rule.as_deref(), Some("backlog"));
        assert_eq!(
            s.created,
            Some(Utc.with_ymd_and_hms(2013, 5, 30, 9, 11, 36).unwrap())
        );
        assert_eq!(
            s.updated,
            Some(Utc.with_ymd_and_hms(2013, 6, 15, 10, 20, 45).unwrap())
        );
    }

    /// 任意フィールドが欠落・null でも通ること。
    ///
    /// 観測は1スペースのみなので、他スペースでの欠落を否定できない。
    #[test]
    fn tolerates_missing_and_null_optional_fields() {
        let absent: Space = serde_json::from_str(r#"{ "spaceKey": "X", "name": "n" }"#).unwrap();
        assert!(absent.owner_id.is_none());
        assert!(absent.lang.is_none());
        assert!(absent.timezone.is_none());
        assert!(absent.report_send_time.is_none());
        assert!(absent.text_formatting_rule.is_none());
        assert!(absent.created.is_none());
        assert!(absent.updated.is_none());

        let nulls: Space = serde_json::from_str(
            r#"{ "spaceKey": "X", "name": "n", "ownerId": null, "lang": null,
                 "timezone": null, "reportSendTime": null, "textFormattingRule": null,
                 "created": null, "updated": null }"#,
        )
        .unwrap();
        assert!(nulls.owner_id.is_none());
        assert!(nulls.lang.is_none());
        assert!(nulls.timezone.is_none());
        assert!(nulls.report_send_time.is_none());
        assert!(nulls.text_formatting_rule.is_none());
        assert!(nulls.created.is_none());
        assert!(nulls.updated.is_none());

        // 必須2つが欠ければ従来どおり失敗する(寛容にしすぎない)
        assert!(serde_json::from_str::<Space>(r#"{ "name": "n" }"#).is_err());
        assert!(serde_json::from_str::<Space>(r#"{ "spaceKey": "X" }"#).is_err());
    }
}
