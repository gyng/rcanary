#[macro_use]
extern crate serde_derive;
extern crate serde;

use std::fmt;

use serde::{Serialize, Serializer};

#[derive(Deserialize, Serialize, Eq, PartialEq, Clone, Debug)]
pub struct CanaryMetricsConfig {
    pub enabled: bool,
}

impl Default for CanaryMetricsConfig {
    fn default() -> Self {
        CanaryMetricsConfig { enabled: false }
    }
}

#[derive(Deserialize, Serialize, Eq, PartialEq, Clone, Debug)]
pub struct CanaryEmailAlertConfig {
    pub alert_email: String,
    pub smtp_server: String,
    pub smtp_username: String,
    pub smtp_password: String,
    pub smtp_port: u16,
}

impl Default for CanaryEmailAlertConfig {
    fn default() -> Self {
        CanaryEmailAlertConfig {
            alert_email: "".to_string(),
            smtp_server: "".to_string(),
            smtp_username: "".to_string(),
            smtp_password: "".to_string(),
            smtp_port: 0,
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
    pub targets: CanaryTargetTypes,
    pub server_listen_address: String,
    pub health_check_address: Option<String>,
    #[serde(default)]
    pub alert: CanaryAlertConfig,
}

#[derive(Deserialize, Serialize, Eq, PartialEq, Clone, Debug)]
pub struct CanaryTargetTypes {
    pub http: Vec<CanaryTarget>,
}

#[derive(Deserialize, Serialize, Eq, PartialEq, Clone, Debug, Hash)]
pub struct CanaryTarget {
    pub name: String,
    pub host: String,
    pub tag: Option<String>,
    pub interval_s: u64,
    pub alert: bool,
    pub basic_auth: Option<Auth>,
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
        s.serialize_str("Auth { ... }")
    }
}

#[derive(Deserialize, Serialize, Eq, PartialEq, Clone, Debug)]
pub struct CanaryCheck {
    pub target: CanaryTarget,
    pub status: Status,
    pub status_code: String,
    pub time: String,
    pub alert: bool,
    pub need_to_alert: bool,
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
