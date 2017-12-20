extern crate docopt;
extern crate env_logger;
extern crate lettre;
#[macro_use]
extern crate log;
extern crate reqwest;
#[macro_use]
extern crate serde_derive;
extern crate serde;
extern crate serde_json;
extern crate time;
extern crate toml;
extern crate ws;
extern crate librcanary;

mod alert;
mod ws_handler;

use std::collections::HashMap;
use std::env;
use std::error::Error;
use std::fs::File;
use std::io::Read;
use std::sync::mpsc;
use std::thread;
use std::time::Duration;

use librcanary::*;
use docopt::Docopt;
use reqwest::header::{Authorization, Basic, Headers, UserAgent};

const USAGE: &'static str = "
rcanary
A minimal monitoring program with email alerts

Usage:
  rcanary <configuration-file>
  rcanary (-h | --help)

Options:
  -h --help     Show this screen.
";

#[derive(Deserialize, Debug)]
struct Args {
    arg_configuration_file: String,
}

fn main() {
    env::set_var("RUST_LOG", "rcanary=info,ws=info"); // TODO: use a proper logger
    env_logger::init().unwrap();

    let args: Args = Docopt::new(USAGE)
        .and_then(|d| d.deserialize())
        .unwrap_or_else(|e| e.exit());

    let config = read_config(&args.arg_configuration_file)
        .map_err(|err| {
            error!(
                "[status.startup] failed to read configuration file {}: {}",
                &args.arg_configuration_file,
                err
            );
            panic!();
        })
        .unwrap();

    // Setup map to save results
    let mut last_statuses = HashMap::new();

    // Start polling
    let (poll_tx, poll_rx) = mpsc::channel();

    for http_target in config.clone().targets.http {
        let child_poll_tx = poll_tx.clone();

        thread::spawn(move || loop {
            let _ = child_poll_tx.send(check_host(&http_target));
            thread::sleep(Duration::new(http_target.interval_s, 0));
        });
    }

    // Start up websocket server
    info!("[status.startup] starting websocker server...");
    let me = ws::WebSocket::new(ws_handler::ClientFactory { config: config.clone() })
        .unwrap_or_else(|err| {
            error!("[status.startup] failed to start websocket server {}", err);
            panic!()
        });
    info!("[status.startup] started websocker server.");
    let broadcaster = me.broadcaster();
    let config_clone = config.clone();
    thread::spawn(move || {
        me.listen(&*config_clone.server_listen_address)
            .unwrap_or_else(|err| {
                error!(
                    "[status.startup] failed to start websocket listener {}",
                    err
                );
                panic!()
            });
    });
    info!("[status.startup] started websocket listener.");

    // Broadcast to all clients
    loop {
        let result = match poll_rx.recv() {
            Ok(result) => result,
            Err(_) => continue,
        };

        info!("[probe.result] {:?}", &result);

        let is_spam = alert::check_spam(&last_statuses, &result);
        let is_fixed = alert::check_fixed(&last_statuses, &result);
        last_statuses.insert(result.target.clone(), result.status.clone());

        if config.alert.enabled && result.alert && (is_fixed || result.need_to_alert && !is_spam) {
            let child_config = config.clone();
            let child_result = result.clone();
            thread::spawn(move || alert::send_alert(&child_config, &child_result));
        }

        if let Ok(json) = serde_json::to_string(&result) {
            let _ = broadcaster.send(json);
        } else {
            error!(
                "[status.startup] failed to encode result into json {:?}",
                &result
            );
        }
    }
}

fn check_host(target: &CanaryTarget) -> CanaryCheck {
    let mut headers = Headers::new();
    headers.set(UserAgent::new("rcanary/0.2.0"));

    if let Some(ref a) = target.basic_auth {
        headers.set(Authorization(Basic {
            username: a.username.clone(),
            password: a.password.clone(),
        }))
    };

    let request = reqwest::Client::new().and_then(|r| r.get(&target.host));

    let (need_to_alert, status, status_code) = match request {
        Ok(mut request) => {
            match request.headers(headers).send() {
                Ok(ref r) if r.status().is_success() => (
                    false,
                    Status::Okay,
                    r.status().to_string(),
                ),
                Ok(ref r) => (true, Status::Fire, r.status().to_string()),
                Err(err) => (
                    true,
                    Status::Unknown,
                    format!("failed to poll server: {}", err),
                ),
            }
        }
        Err(err) => (false, Status::Unknown, format!("bad URL: {}", err)),
    };

    CanaryCheck {
        target: target.clone(),
        time: format!("{}", time::now_utc().rfc3339()),
        status: status,
        status_code: status_code,
        alert: target.alert,
        need_to_alert: need_to_alert,
    }
}

fn read_config(path: &str) -> Result<CanaryConfig, Box<Error>> {
    info!("[status.startup] reading configuration from `{}`...", path);
    let mut file = File::open(&path)?;
    let mut config_toml = String::new();
    file.read_to_string(&mut config_toml)?;
    info!("[status.startup] read configuration file.");

    Ok(toml::from_str(&config_toml)?)
}

#[cfg(test)]
mod tests {
    extern crate hyper;
    extern crate service_fn;

    use super::*;

    use std::{thread, time};
    use self::hyper::server::{Http, Request, Response};
    use self::hyper::header::{ContentLength, ContentType};
    use self::service_fn::service_fn;

    fn sleep() {
        let delay = time::Duration::from_millis(250);
        thread::sleep(delay);
    }

    pub fn target() -> CanaryTarget {
        CanaryTarget {
            name: "foo".to_string(),
            host: "invalid".to_string(),
            tag: Some("tag".to_string()),
            interval_s: 1,
            alert: false,
            basic_auth: None,
        }
    }

    #[test]
    fn it_reads_and_parses_a_config_file() {
        let expected = CanaryConfig {
            alert: CanaryAlertConfig {
                enabled: true,
                alert_email: "rcanary.alert.inbox@gmail.com".to_string(),
                smtp_server: "smtp.googlemail.com".to_string(),
                smtp_username: "example@gmail.com".to_string(),
                smtp_password: "hunter2".to_string(),
                smtp_port: 587,
            },
            server_listen_address: "127.0.0.1:8099".to_string(),
            targets: CanaryTargetTypes {
                http: vec![
                    CanaryTarget {
                        name: "Invalid".to_string(),
                        host: "Hello, world!".to_string(),
                        tag: None,
                        interval_s: 60,
                        alert: false,
                        basic_auth: None,
                    },
                    CanaryTarget {
                        name: "404".to_string(),
                        host: "http://www.google.com/404".to_string(),
                        tag: Some("example-tag".to_string()),
                        interval_s: 5,
                        alert: false,
                        basic_auth: None,
                    },
                    CanaryTarget {
                        name: "Google".to_string(),
                        host: "https://www.google.com".to_string(),
                        tag: None,
                        interval_s: 5,
                        alert: false,
                        basic_auth: Some(Auth {
                            username: "AzureDiamond".to_string(),
                            password: Some("hunter2".to_string()),
                        }),
                    },
                ],
            },
        };

        let actual = read_config("tests/fixtures/config.toml").unwrap();

        assert_eq!(expected, actual);
    }

    #[test]
    fn it_checks_invalid_target_hosts() {
        let actual = check_host(&target());

        let expected = CanaryCheck {
            target: target(),
            time: actual.time.clone(),
            status: Status::Unknown,
            status_code: "bad URL: relative URL without a base".to_string(),
            alert: false,
            need_to_alert: false,
        };

        assert_eq!(expected, actual);
    }

    #[test]
    fn it_checks_valid_target_hosts() {
        static TEXT: &'static str = "I love BGP";
        thread::spawn(move || {
            let addr = ([127, 0, 0, 1], 56473).into();
            let hello = || {
                Ok(service_fn(|_req| {
                    Ok(
                        Response::<hyper::Body>::new()
                            .with_header(ContentLength(TEXT.len() as u64))
                            .with_header(ContentType::plaintext())
                            .with_body(TEXT),
                    )
                }))
            };
            let server = Http::new().bind(&addr, hello).unwrap();
            let _ = server.run();
        });
        sleep();

        let ok_target = CanaryTarget {
            name: "foo".to_string(),
            host: "http://127.0.0.1:56473".to_string(),
            tag: Some("bar".to_string()),
            interval_s: 1,
            alert: false,
            basic_auth: None,
        };

        let ok_actual = check_host(&ok_target);

        let ok_expected = CanaryCheck {
            target: ok_target.clone(),
            time: ok_actual.time.clone(),
            status: Status::Okay,
            status_code: "200 OK".to_string(),
            alert: false,
            need_to_alert: false,
        };

        assert_eq!(ok_expected, ok_actual);
    }

    #[test]
    fn it_checks_valid_target_hosts_with_basic_auth() {
        thread::spawn(move || {
            let addr = ([127, 0, 0, 1], 56474).into();
            let hello = || {
                Ok(service_fn(|req: Request| {
                    assert!(
                        req.headers()
                            .to_string()
                            .find("Basic QXp1cmVEaWFtb25kOmh1bnRlcjI=") // hunter2
                            .is_some()
                    );
                    Ok(Response::<hyper::Body>::new())
                }))
            };
            let server = Http::new().bind(&addr, hello).unwrap();
            let _ = server.run();
        });
        sleep();

        let ok_target = CanaryTarget {
            name: "foo".to_string(),
            host: "http://127.0.0.1:56474".to_string(),
            tag: Some("bar".to_string()),
            interval_s: 1,
            alert: false,
            basic_auth: Some(Auth {
                username: "AzureDiamond".to_string(),
                password: Some("hunter2".to_string()),
            }),
        };

        let ok_actual = check_host(&ok_target);

        let ok_expected = CanaryCheck {
            target: ok_target.clone(),
            time: ok_actual.time.clone(),
            status: Status::Okay,
            status_code: "200 OK".to_string(),
            alert: false,
            need_to_alert: false,
        };

        assert_eq!(ok_expected, ok_actual);
    }
}
