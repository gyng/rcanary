#[macro_use]
extern crate serde_derive;
extern crate serde;

use std::fmt;

use serde::{Serialize, Serializer};

#[derive(Deserialize, Serialize, Eq, PartialEq, Clone, Debug)]
pub struct CanaryMetricsConfig {
    pub address: String,
    pub enabled: bool,
}

impl Default for CanaryMetricsConfig {
    fn default() -> Self {
        CanaryMetricsConfig {
            address: "".to_string(),
            enabled: false,
        }
    }
}

#[derive(Deserialize, Serialize, Eq, PartialEq, Clone, Debug)]
pub struct CanaryHealthCheckConfig {
    pub address: String,
    pub enabled: bool,
}

impl Default for CanaryHealthCheckConfig {
    fn default() -> Self {
        CanaryHealthCheckConfig {
            address: "".to_string(),
            enabled: false,
        }
    }
}

#[derive(Deserialize, Serialize, Eq, PartialEq, Clone, Debug)]
pub struct CanaryEmailAlertConfig {
    pub alert_email: String,
    pub smtp_server: String,
    pub smtp_username: String,
    pub smtp_password: String,
}

impl Default for CanaryEmailAlertConfig {
    fn default() -> Self {
        CanaryEmailAlertConfig {
            alert_email: "".to_string(),
            smtp_server: "".to_string(),
            smtp_username: "".to_string(),
            smtp_password: "".to_string(),
        }
    }
}

#[derive(Deserialize, Serialize, Eq, PartialEq, Clone, Debug)]
pub struct CanaryAlertConfig {
    pub enabled: bool,
    pub email: Option<CanaryEmailAlertConfig>,
}

impl Default for CanaryAlertConfig {
    fn default() -> Self {
        CanaryAlertConfig {
            enabled: false,
            email: None,
        }
    }
}

#[derive(Deserialize, Serialize, Eq, PartialEq, Clone, Debug)]
pub struct CanaryConfig {
    #[serde(default)]
    pub alert: CanaryAlertConfig,
    #[serde(default)]
    pub health_check: Option<CanaryHealthCheckConfig>,
    #[serde(default)]
    pub metrics: Option<CanaryMetricsConfig>,
    pub server_listen_address: String,
    pub targets: CanaryTargetTypes,
}

#[derive(Deserialize, Serialize, Eq, PartialEq, Clone, Debug)]
pub struct CanaryTargetTypes {
    pub http: Vec<CanaryTarget>,
}

#[derive(Deserialize, Serialize, Eq, PartialEq, Clone, Debug, Hash)]
pub struct CanaryTarget {
    pub alert: bool,
    pub basic_auth: Option<Auth>,
    pub host: String,
    pub interval_s: u64,
    pub name: String,
    pub tag_metric: Option<String>,
    pub tag: Option<String>,
}

#[derive(Deserialize, Eq, PartialEq, Clone, Hash)]
pub struct Auth {
    pub username: String,
    pub password: Option<String>,
}

impl fmt::Debug for Auth {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "Auth {{ ... }}")
    }
}

impl Serialize for Auth {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_str("redacted")
    }
}

#[derive(Deserialize, Serialize, Eq, PartialEq, Clone, Debug)]
pub struct CanaryCheck {
    pub alert: bool,
    pub latency_ms: u64,
    pub need_to_alert: bool,
    pub status_code: String,
    pub status_reason: String,
    pub status: Status,
    pub target: CanaryTarget,
    pub time: String,
}

#[derive(Deserialize, Serialize, Eq, PartialEq, Clone, Debug)]
pub enum Status {
    Okay,
    Fire,
    Unknown,
}

#[cfg(test)]
mod tests {
    extern crate serde_json;

    use super::Auth;

    #[test]
    fn it_does_not_leak_passwords_in_debug_representation() {
        let auth = Auth {
            username: "AzureDiamond".to_string(),
            password: Some("hunter2".to_string()),
        };

        let formatted = format!("{:?}", auth);

        assert!(formatted.find("AzureDiamond").is_none());
        assert!(formatted.find("hunter2").is_none());
    }

    #[test]
    fn it_does_not_leak_passwords_in_encodable_representation() {
        let auth = Auth {
            username: "AzureDiamond".to_string(),
            password: Some("hunter2".to_string()),
        };

        let encoded = serde_json::to_string(&auth).unwrap();

        assert!(encoded.find("AzureDiamond").is_none());
        assert!(encoded.find("hunter2").is_none());
    }
}
