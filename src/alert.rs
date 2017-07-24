use std::collections::HashMap;

use CanaryCheck;
use CanaryConfig;
use CanaryTarget;
use Status;

use lettre::email::EmailBuilder;
use lettre::transport::EmailTransport;
use lettre::transport::smtp::{SecurityLevel, SmtpTransportBuilder};

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
        (Some(&Status::Fire), &Status::Okay) |
        (Some(&Status::Unknown), &Status::Okay) => true,
        _ => false,
    }
}

pub fn send_alert(config: &CanaryConfig, result: &CanaryCheck) -> Result<(), String> {
    info!("[alert.send] sending alert for {:?}", result);

    let body = match result.status {
        Status::Fire => format!("🔥 Something has gone terribly wrong:\n{:#?}", result),
        Status::Unknown => format!("🚨 Something is probably wrong:\n{:#?}", result),
        Status::Okay => format!("🙇 Everything is now okay:\n{:#?}", result),
    };

    let email = match EmailBuilder::new()
        .to(&*config.alert.alert_email)
        .from(&*config.alert.smtp_username)
        .subject(&format!("rcanary alert for {}", &result.target.host))
        .body(&body)
        .build() {
        Ok(e) => e,
        Err(err) => return Err(format!("{}", err)),
    };

    let transport = SmtpTransportBuilder::new((&*config.alert.smtp_server, config.alert.smtp_port));
    let mut mailer = match transport {
        Ok(t) => {
            t.hello_name("localhost")
                .credentials(&config.alert.smtp_username, &config.alert.smtp_password)
                .security_level(SecurityLevel::AlwaysEncrypt)
                .smtp_utf8(true)
                .build()
        }
        Err(err) => {
            return Err(format!("failed to create email smtp transport for {} {}: {}",
                               config.alert.smtp_server,
                               config.alert.smtp_port,
                               err))
        }
    };

    match mailer.send(email) {
        Ok(_) => {
            info!("[alert.success] email alert sent to {} for {}",
                  config.alert.alert_email,
                  &result.target.host);
            Ok(())
        }
        Err(err) => {
            let error_string = format!("[alert.failure] failed to send email alert: {}", err);
            info!("{}", error_string);
            Err(error_string)
        }
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;
    use super::*;
    use {CanaryCheck, Status};
    use tests::target;

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
