use super::Alerter;
use librcanary::{CanaryCheck, CanaryConfig, Status};

use lettre::email::EmailBuilder;
use lettre::transport::smtp::{SecurityLevel, SmtpTransportBuilder};
use lettre::transport::EmailTransport;

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
