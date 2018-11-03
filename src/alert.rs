use std::collections::HashMap;

use crate::CanaryCheck;
use crate::CanaryConfig;
use crate::CanaryTarget;
use crate::Status;

use lettre::email::EmailBuilder;
use lettre::transport::smtp::{SecurityLevel, SmtpTransportBuilder};
use lettre::transport::EmailTransport;

// Checks if alert would be spam.
// Alert would be spam if state has not changed since last poll
pub fn check_spam(last_statuses: &HashMap<CanaryTarget, Status>, result: &CanaryCheck) -> bool {
    match last_statuses.get(&result.target) {
        Some(status) => status == &result.status,
        _ => false,
    }
}

pub fn check_fixed(last_statuses: &HashMap<CanaryTarget, Status>, result: &CanaryCheck) -> bool {
    match (last_statuses.get(&result.target), &result.status) {
        (Some(&Status::Fire), &Status::Okay) | (Some(&Status::Unknown), &Status::Okay) => true,
        _ => false,
    }
}

pub trait Alerter {
    fn alert(&self, result: &CanaryCheck) -> Result<(), String>;
}

pub struct EmailAlerter<'a> {
    pub config: &'a CanaryConfig,
}
impl<'a> Alerter for EmailAlerter<'a> {
    fn alert(&self, result: &CanaryCheck) -> Result<(), String> {
        let body = match result.status {
            Status::Fire => format!("🔥 Something has gone terribly wrong:\n{:#?}", result),
            Status::Unknown => format!("🚨 Something is probably wrong:\n{:#?}", result),
            Status::Okay => format!("🙇 Everything is now okay:\n{:#?}", result),
        };

        let email_config = match self.config.alert.email {
            Some(ref config) => config,
            None => return Err("email alerts configuration missing".to_string()),
        };

        let email = match EmailBuilder::new()
            .to(&*email_config.alert_email)
            .from(&*email_config.smtp_username)
            .subject(&format!("rcanary alert for {}", &result.target.host))
            .body(&body)
            .build()
        {
            Ok(e) => e,
            Err(err) => return Err(format!("{}", err)),
        };

        let transport =
            SmtpTransportBuilder::new((&*email_config.smtp_server, email_config.smtp_port));
        let mut mailer = match transport {
            Ok(t) => t
                .hello_name("localhost")
                .credentials(&email_config.smtp_username, &email_config.smtp_password)
                .security_level(SecurityLevel::AlwaysEncrypt)
                .smtp_utf8(true)
                .build(),
            Err(err) => {
                return Err(format!(
                    "failed to create email smtp transport for {} {}: {}",
                    email_config.smtp_server, email_config.smtp_port, err
                ))
            }
        };

        match mailer.send(email) {
            Ok(_) => {
                info!(
                    "[alert.success] email alert sent to {} for {}",
                    email_config.alert_email, &result.target.host
                );
                Ok(())
            }
            Err(err) => {
                let error_string = format!("[alert.failure] failed to send email alert: {}", err);
                info!("{}", error_string);
                Err(error_string)
            }
        }
    }
}

pub fn send_alert(config: &CanaryConfig, result: &CanaryCheck) -> Result<(), String> {
    info!("[alert.send] sending alert for {:?}", result);

    if config.alert.email.is_some() {
        let alerter: EmailAlerter = EmailAlerter { config: &config };
        return alerter.alert(result);
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tests::target;
    use crate::{CanaryCheck, Status};
    use std::collections::HashMap;

    fn okay_result() -> CanaryCheck {
        CanaryCheck {
            target: target(),
            time: "2016-10-14T08:00:00Z".to_string(),
            status: Status::Okay,
            status_code: "200 OK".to_string(),
            alert: true,
            need_to_alert: true,
        }
    }

    fn fire_result() -> CanaryCheck {
        CanaryCheck {
            target: target(),
            time: "2016-10-14T08:00:00Z".to_string(),
            status: Status::Fire,
            status_code: "401 Unauthorized".to_string(),
            alert: true,
            need_to_alert: true,
        }
    }

    #[test]
    fn it_marks_as_spam_on_empty_history() {
        let mut last_statuses = HashMap::new();

        let actual = check_spam(&mut last_statuses, &okay_result());

        assert_eq!(false, actual);
    }

    #[test]
    fn it_does_not_mark_as_spam_on_change_from_okay_to_fire() {
        let mut last_statuses = HashMap::new();
        last_statuses.insert(target(), Status::Okay);

        let actual = check_spam(&mut last_statuses, &fire_result());

        assert_eq!(false, actual);
    }

    #[test]
    fn it_marks_as_spam_on_continued_okay() {
        let mut last_statuses = HashMap::new();
        last_statuses.insert(target(), Status::Okay);

        let actual = check_spam(&mut last_statuses, &okay_result());

        assert_eq!(true, actual);
    }

    #[test]
    fn it_marks_as_spam_on_continued_fire() {
        let mut last_statuses = HashMap::new();
        last_statuses.insert(target(), Status::Fire);

        let actual = check_spam(&mut last_statuses, &fire_result());

        assert_eq!(true, actual);
    }

    #[test]
    fn it_does_not_mark_as_spam_on_change_from_fire_to_okay() {
        let mut last_statuses = HashMap::new();
        last_statuses.insert(target(), Status::Fire);

        let actual = check_spam(&mut last_statuses, &okay_result());

        assert_eq!(false, actual);
    }

    #[test]
    fn it_marks_as_spam_on_change_from_unknown_to_fire() {
        let mut last_statuses = HashMap::new();
        last_statuses.insert(target(), Status::Unknown);

        let actual = check_spam(&mut last_statuses, &fire_result());

        assert_eq!(false, actual);
    }

    #[test]
    fn it_marks_as_fixed_on_change_from_unknown_to_okay() {
        let mut last_statuses = HashMap::new();
        last_statuses.insert(target(), Status::Unknown);

        let actual = check_fixed(&mut last_statuses, &okay_result());

        assert_eq!(true, actual);
    }

    #[test]
    fn it_marks_as_fixed_on_change_from_fire_to_okay() {
        let mut last_statuses = HashMap::new();
        last_statuses.insert(target(), Status::Fire);

        let actual = check_fixed(&mut last_statuses, &okay_result());

        assert_eq!(true, actual);
    }

    #[test]
    fn it_marks_as_unfixed_on_change_from_fire_to_fire() {
        let mut last_statuses = HashMap::new();
        last_statuses.insert(target(), Status::Fire);

        let actual = check_fixed(&mut last_statuses, &fire_result());

        assert_eq!(false, actual);
    }
}
