use std::collections::HashMap;

use alerter::email::EmailAlerter;
use alerter::Alerter;

use CanaryCheck;
use CanaryConfig;
use CanaryTarget;
use Status;

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
    use std::collections::HashMap;
    use tests::target;
    use {CanaryCheck, Status};

    fn okay_result() -> CanaryCheck {
        CanaryCheck {
            alert: true,
            latency_ms: 299,
            need_to_alert: true,
            status_code: "200 OK".to_string(),
            status: Status::Okay,
            status_reason: "no reason".to_string(),
            target: target(),
            time: "2016-10-14T08:00:00Z".to_string(),
        }
    }

    fn fire_result() -> CanaryCheck {
        CanaryCheck {
            alert: true,
            latency_ms: 499,
            need_to_alert: true,
            status_code: "401 Unauthorized".to_string(),
            status: Status::Fire,
            status_reason: "no reason".to_string(),
            target: target(),
            time: "2016-10-14T08:00:00Z".to_string(),
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
