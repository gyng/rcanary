use super::Alerter;
use librcanary::{CanaryCheck, CanaryConfig, Status};

use lettre::smtp::authentication::{Credentials, Mechanism};
use lettre::smtp::extension::ClientId;
use lettre::smtp::ConnectionReuseParameters;
use lettre::{SmtpClient, Transport};
use lettre_email::Email;

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

        let email = Email::builder()
            .to(&*email_config.alert_email)
            .from(&*email_config.smtp_username)
            .subject(&format!("rcanary alert for {}", &result.target.host))
            .text(&body)
            .build()
            .unwrap();

        let mut mailer = SmtpClient::new_simple(&*email_config.smtp_server)
            .unwrap()
            .hello_name(ClientId::Domain("localhost".to_string()))
            .credentials(Credentials::new(
                email_config.smtp_username.clone(),
                email_config.smtp_password.clone(),
            ))
            .smtp_utf8(true)
            .authentication_mechanism(Mechanism::Plain)
            .connection_reuse(ConnectionReuseParameters::ReuseUnlimited)
            .transport();

        match mailer.send(email.into()) {
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
