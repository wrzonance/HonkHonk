use std::collections::BTreeMap;

use serde::de::Deserializer;
use serde::{Deserialize, Serialize};

#[cfg(test)]
use super::AppConfig;

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct SortPref {
    #[serde(default)]
    key: String,
    #[serde(default)]
    direction: String,
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    group_by_tag: bool,
}

impl SortPref {
    pub fn new(key: impl Into<String>, direction: impl Into<String>) -> Self {
        Self {
            key: key.into(),
            direction: direction.into(),
            group_by_tag: false,
        }
    }

    pub fn key(&self) -> &str {
        &self.key
    }

    pub fn group_by_tag(&self) -> bool {
        self.group_by_tag
    }

    pub fn with_tag_grouping(mut self, enabled: bool) -> Self {
        self.group_by_tag = enabled;
        self
    }

    pub fn direction(&self) -> &str {
        &self.direction
    }
}

pub(super) fn deserialize_sort_prefs<'de, D>(
    deserializer: D,
) -> Result<BTreeMap<String, SortPref>, D::Error>
where
    D: Deserializer<'de>,
{
    let value = serde_json::Value::deserialize(deserializer)?;
    let Some(entries) = value.as_object() else {
        return Ok(BTreeMap::new());
    };

    Ok(entries
        .iter()
        .filter_map(|(view, value)| parse_sort_pref(value).map(|pref| (view.clone(), pref)))
        .collect())
}

fn parse_sort_pref(value: &serde_json::Value) -> Option<SortPref> {
    let fields = value.as_object()?;
    let key = optional_string(fields.get("key"))?;
    let direction = optional_string(fields.get("direction"))?;
    let grouped = fields
        .get("group_by_tag")
        .and_then(serde_json::Value::as_bool)
        .unwrap_or(false);
    Some(SortPref::new(key, direction).with_tag_grouping(grouped))
}

fn optional_string(value: Option<&serde_json::Value>) -> Option<&str> {
    match value {
        None => Some(""),
        Some(serde_json::Value::String(value)) => Some(value),
        Some(_) => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sort_preferences_round_trip_through_app_config() {
        let mut config = AppConfig::default();
        config
            .sort_prefs
            .insert("tiles".into(), SortPref::new("modified", "descending"));

        let json = serde_json::to_string(&config).unwrap();
        let loaded: AppConfig = serde_json::from_str(&json).unwrap();

        assert_eq!(loaded.sort_prefs, config.sort_prefs);
    }

    #[test]
    fn unknown_preference_data_does_not_reject_config() {
        let mut config = AppConfig::default();
        config
            .sort_prefs
            .insert("tiles".into(), SortPref::new("future-key", "sideways"));

        let json = serde_json::to_string(&config).unwrap();
        let loaded: AppConfig = serde_json::from_str(&json).unwrap();
        let pref = loaded.sort_prefs.get("tiles").unwrap();

        assert_eq!(pref.key(), "future-key");
        assert_eq!(pref.direction(), "sideways");
    }

    #[test]
    fn missing_sort_preferences_use_empty_default() {
        let json = serde_json::to_value(AppConfig::default()).unwrap();
        let mut object = json.as_object().unwrap().clone();
        object.remove("sort_prefs");

        let loaded: AppConfig = serde_json::from_value(serde_json::Value::Object(object)).unwrap();

        assert!(loaded.sort_prefs.is_empty());
    }

    #[test]
    fn incomplete_preference_data_does_not_reject_config() {
        let mut value = serde_json::to_value(AppConfig::default()).unwrap();
        value["sort_prefs"] = serde_json::json!({"tiles": {}});

        let loaded: AppConfig = serde_json::from_value(value).unwrap();
        let pref = loaded.sort_prefs.get("tiles").unwrap();

        assert_eq!(pref, &SortPref::default());
    }

    #[test]
    fn malformed_sort_preferences_field_uses_empty_default() {
        for malformed in [
            serde_json::Value::Null,
            serde_json::json!([]),
            serde_json::json!("not-a-map"),
            serde_json::json!(42),
        ] {
            let mut value = serde_json::to_value(AppConfig::default()).unwrap();
            value["sort_prefs"] = malformed;

            let loaded: AppConfig = serde_json::from_value(value).unwrap();

            assert!(loaded.sort_prefs.is_empty());
        }
    }

    #[test]
    fn malformed_entries_are_skipped_without_losing_valid_preferences() {
        let mut value = serde_json::to_value(AppConfig::default()).unwrap();
        value["sort_prefs"] = serde_json::json!({
            "tiles": {"key": "added", "direction": "descending"},
            "future-null": null,
            "future-number": 42,
            "future-bad-key": {"key": ["name"], "direction": "ascending"},
            "future-bad-direction": {"key": "name", "direction": false}
        });

        let loaded: AppConfig = serde_json::from_value(value).unwrap();

        assert_eq!(
            loaded.sort_prefs.get("tiles"),
            Some(&SortPref::new("added", "descending"))
        );
        assert_eq!(loaded.sort_prefs.len(), 1);
    }

    #[test]
    fn tolerant_sort_preferences_do_not_weaken_other_config_validation() {
        let mut value = serde_json::to_value(AppConfig::default()).unwrap();
        value["volume"] = serde_json::json!("loud");
        value["sort_prefs"] = serde_json::json!({"future": null});

        assert!(serde_json::from_value::<AppConfig>(value).is_err());
    }
}
