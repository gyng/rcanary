extern crate docopt;
extern crate env_logger;
extern crate hyper;
extern crate lettre;
#[macro_use]
extern crate log;
extern crate rustc_serialize;
extern crate time;
extern crate toml;
extern crate ws;

mod alert;
mod ws_handler;

use std::collections::HashMap;
use std::env;
use std::error::Error;
use std::fs::File;
use std::io::Read;
use std::result::Result;
use std::sync::mpsc;
use std::thread;
use std::time::Duration;

use docopt::Docopt;
use rustc_serialize::json;

#[derive(RustcDecodable, RustcEncodable, Eq, PartialEq, Clone, Debug)]
pub struct CanaryAlertConfig {
    enabled: bool,
    alert_email: String,
    smtp_server: String,
    smtp_username: String,
    smtp_password: String,
    smtp_port: u16,
}

#[derive(RustcDecodable, RustcEncodable, Eq, PartialEq, Clone, Debug)]
pub struct CanaryConfig {
    targets: CanaryTargetTypes,
    server_listen_address: String,
    alert: CanaryAlertConfig,
}

#[derive(RustcDecodable, RustcEncodable, Eq, PartialEq, Clone, Debug)]
struct CanaryTargetTypes {
    http: Vec<CanaryTarget>,
}

#[derive(RustcDecodable, RustcEncodable, Eq, PartialEq, Clone, Debug, Hash)]
pub struct CanaryTarget {
    name: String,
    host: String,
    interval_s: u64,
    alert: bool,
}

#[derive(RustcEncodable, Eq, PartialEq, Clone, Debug)]
pub struct CanaryCheck {
    target: CanaryTarget,
    status: Status,
    status_code: String,
    time: String,
    alert: bool,
    need_to_alert: bool,
}

#[derive(RustcEncodable, Eq, PartialEq, Clone, Debug)]
pub enum Status {
    Okay,
    Fire,
    Unknown,
}

const USAGE: &'static str = "
rcanary
A minimal monitoring program with email alerts

Usage:
  rcanary <configuration-file>
  rcanary (-h | --help)

Options:
  -h --help     Show this screen.
";

#[derive(RustcDecodable, Debug)]
struct Args {
    arg_configuration_file: String,
}

fn main() {
    env::set_var("RUST_LOG", "rcanary=info,ws=info"); // TODO: use a proper logger
    env_logger::init().unwrap();

    let args: Args = Docopt::new(USAGE)
        .and_then(|d| d.decode())
        .unwrap_or_else(|e| e.exit());

    let config = read_config(&args.arg_configuration_file)
        .map_err(|err| {
            panic!("failed to read configuration file {}: {}",
                   &args.arg_configuration_file,
                   err)
        })
        .unwrap();

    // Setup map to save results
    let mut last_statuses = HashMap::new();

    // Start polling
    let (poll_tx, poll_rx) = mpsc::channel();

    for http_target in config.clone().targets.http {
        let child_poll_tx = poll_tx.clone();

        thread::spawn(move || {
            loop {
                let _ = child_poll_tx.send(check_host(&http_target));
                thread::sleep(Duration::new(http_target.interval_s, 0));
            }
        });
    }

    // Start up websocket server
    let me = ws::WebSocket::new(ws_handler::ClientFactory { config: config.clone() })
        .unwrap_or_else(|err| panic!("failed to start websocket server {}", err));
    let broadcaster = me.broadcaster();
    let config_clone = config.clone();
    thread::spawn(move || {
        me.listen(&*config_clone.server_listen_address)
            .unwrap_or_else(|err| panic!("failed to start websocket listener {}", err));
    });

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

        if let Ok(json) = json::encode(&result) {
            let _ = broadcaster.send(json);
        } else {
            error!("failed to encode result into json {:?}", &result);
        }
    }
}

fn check_host(target: &CanaryTarget) -> CanaryCheck {
    let response_raw = hyper::Client::new().get(&target.host).send();

    let (need_to_alert, status, status_code) = match response_raw {
        Ok(ref r) if r.status.is_success() => (false, Status::Okay, r.status.to_string()),
        Ok(ref r) => (true, Status::Fire, r.status.to_string()),
        Err(err) => (true, Status::Unknown, format!("failed to poll server: {}", err)),
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
    info!("reading configuration from `{}`...", path);
    let mut file = File::open(&path)?;
    let mut config_toml = String::new();
    file.read_to_string(&mut config_toml)?;

    let parsed_toml = toml::Parser::new(&config_toml)
        .parse()
        .expect("error parsing config file");

    let config = toml::Value::Table(parsed_toml);
    toml::decode(config).ok_or_else(|| panic!("error deserializing config"))
}

#[cfg(test)]
mod tests {
    extern crate hyper;

    use std::thread;
    use super::{CanaryConfig, CanaryAlertConfig, CanaryCheck, CanaryTargetTypes, CanaryTarget,
                Status, read_config, check_host};
    use hyper::server::{Server, Request, Response};

    pub fn target() -> CanaryTarget {
        CanaryTarget {
            name: "foo".to_string(),
            host: "invalid".to_string(),
            interval_s: 1,
            alert: false,
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
                http: vec![CanaryTarget {
                               name: "Invalid".to_string(),
                               host: "Hello, world!".to_string(),
                               interval_s: 60,
                               alert: false,
                           },
                           CanaryTarget {
                               name: "404".to_string(),
                               host: "http://www.google.com/404".to_string(),
                               interval_s: 5,
                               alert: false,
                           },
                           CanaryTarget {
                               name: "Google".to_string(),
                               host: "https://www.google.com".to_string(),
                               interval_s: 5,
                               alert: false,
                           }],
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
            status_code: "failed to poll server: relative URL without a base".to_string(),
            alert: false,
            need_to_alert: true,
        };

        assert_eq!(expected, actual);
    }

    #[test]
    fn it_checks_valid_target_hosts() {
        thread::spawn(move || {
            Server::http("127.0.0.1:56473")
                .unwrap()
                .handle(move |_req: Request, res: Response| {
                    res.send(b"I love BGP").unwrap();
                })
                .unwrap();
        });

        let ok_target = CanaryTarget {
            name: "foo".to_string(),
            host: "http://127.0.0.1:56473".to_string(),
            interval_s: 1,
            alert: false,
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
