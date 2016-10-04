extern crate hyper;
extern crate lettre;
extern crate rustc_serialize;
extern crate time;
extern crate toml;
extern crate ws;

mod ws_handler;

use std::env;
use std::fs::{File, OpenOptions};
use std::fs;
use std::io::{Read, Write};
use std::path::{PathBuf};
use std::result::Result;
use std::sync::mpsc;
use std::thread;
use std::time::Duration;

use lettre::email::EmailBuilder;
use lettre::transport::EmailTransport;
use lettre::transport::smtp::{SecurityLevel, SmtpTransportBuilder};
use rustc_serialize::json;

#[derive(RustcDecodable, RustcEncodable, Eq, PartialEq, Clone, Debug)]
pub struct CanaryLogConfig {
    dir_path: String,
    enabled: bool,
    file: bool,
    stdout: bool
}

#[derive(RustcDecodable, RustcEncodable, Eq, PartialEq, Clone, Debug)]
pub struct CanaryAlertConfig {
    enabled: bool,
    alert_email: String,
    smtp_server: String,
    smtp_username: String,
    smtp_password: String,
    smtp_port: u16
}

#[derive(RustcDecodable, RustcEncodable, Eq, PartialEq, Clone, Debug)]
pub struct CanaryConfig {
    targets: CanaryTargetTypes,
    server_listen_address: String,
    log: CanaryLogConfig,
    alert: CanaryAlertConfig
}

#[derive(RustcDecodable, RustcEncodable, Eq, PartialEq, Clone, Debug)]
struct CanaryTargetTypes {
    http: Vec<CanaryTarget>
}

#[derive(RustcDecodable, RustcEncodable, Eq, PartialEq, Clone, Debug)]
struct CanaryTarget {
    name: String,
    host: String,
    interval_s: u64,
    alert: bool
}

#[derive(RustcEncodable, Eq, PartialEq, Clone, Debug)]
pub struct CanaryCheck {
    target: CanaryTarget,
    info: String,
    status_code: String,
    time: String,
    alert: bool,
    need_to_alert: bool
}

fn main() {
    // Read config
    let config_path = env::args().nth(1)
        .expect("no configuration file supplied as the first argument");

    let config = match read_config(&config_path) {
        Ok(c) => c,
        Err(err) => panic!("failed to read configuration file {}: {}", config_path, err)
    };

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
        me.listen(config_clone.server_listen_address.as_str())
            .unwrap_or_else(|err| panic!("failed to start websocket listener {}", err));
    });

    // Create string reference to previous status code.
    let mut previous_status_code = "".to_string();

    // Broadcast to all clients
    loop {
        let result = match poll_rx.recv() {
            Ok(result) => result,
            Err(_) => continue
        };

        if config.log.enabled {
            log_result(&config.log, &result);
        }

        if config.alert.enabled && result.alert && result.need_to_alert && (previous_status_code != result.status_code) {
            let _ = send_alert(&config.alert, &result);
        }

        previous_status_code = result.status_code.to_string();

        if let Ok(json) = json::encode(&result) {
            let _ = broadcaster.send(json);
        } else {
            println!("failed to encode result into json");
        }
    }
}

fn check_host(target: &CanaryTarget) -> CanaryCheck {
    let response_raw = hyper::Client::new().get(&target.host).send();
    let time = format!("{}", time::now_utc().rfc3339());

    if let Err(err) = response_raw {
        return CanaryCheck {
            target: target.clone(),
            time: time,
            info: "unknown".to_string(),
            status_code: format!("failed to poll server: {}", err),
            alert: target.alert,
            need_to_alert: true
        }
    }

    // Should never panic on unwrap
    let response = response_raw.unwrap();
    let (need_to_alert, info) = if response.status.is_success() {
        (false, "okay".to_string())
    } else {
        (true, "fire".to_string())
    };

    CanaryCheck {
        target: target.clone(),
        time: format!("{}", time::now_utc().rfc3339()),
        info: info,
        status_code: format!("{}", response.status),
        alert: target.alert,
        need_to_alert: need_to_alert
    }
}

fn send_alert(config: &CanaryAlertConfig, result: &CanaryCheck) -> Result<(), String>{
    let email = try!(EmailBuilder::new()
        .to(config.alert_email.as_ref())
        .from(config.smtp_username.as_ref())
        .subject(format!("rcanary alert for {}", &result.target.host).as_str())
        .body(format!("Something has gone terribly wrong:\n{:#?}", result).as_str())
        .build());

    let transport = SmtpTransportBuilder::new((config.smtp_server.as_str(), config.smtp_port));
    let mut mailer = match transport {
        Ok(t) => t
            .hello_name("localhost")
            .credentials(&config.smtp_username, &config.smtp_password)
            .security_level(SecurityLevel::AlwaysEncrypt)
            .smtp_utf8(true)
            .build(),
        Err(err) => return Err(format!("failed to create email smtp transport for {} {}: {}", config.smtp_server, config.smtp_port, err))
    };

    match mailer.send(email.clone()) {
        Ok(_) => {
            println!("email alert sent to {} for {}", config.alert_email, &result.target.host);
            Ok(())
        },
        Err(err) => {
            let error_string = format!("failed to send email alert: {}", err);
            println!("{}", error_string);
            Err(error_string)
        }
    }
}

fn log_result(config: &CanaryLogConfig, result: &CanaryCheck) {
    let log_text = format!("{:?}", result);

    if config.file {
        let mut path_buf = PathBuf::from(&config.dir_path);
        fs::create_dir_all(&path_buf).expect(format!("failed to create directory {}", config.dir_path).as_str());
        path_buf.push("log.txt");
        let mut f = OpenOptions::new()
            .write(true).append(true).create(true)
            .open(path_buf).expect("failed to open log file for writing");

        let _ = f.write_all(log_text.as_bytes());
    }

    if config.stdout {
        println!("{}", log_text);
    }
}

fn read_config(path: &str) -> Result<CanaryConfig, String> {
    println!("reading configuration from `{}`...", path);

    let mut file = match File::open(&path) {
        Ok(f) => f,
        Err(err) => return Err(format!("failed to read file {}", err))
    };

    let mut config_toml = String::new();
    if let Err(err) = file.read_to_string(&mut config_toml) {
        return Err(format!("error reading config: {}", err))
    }

    let parsed_toml = toml::Parser::new(&config_toml).parse().expect("error parsing config file");
    println!("configuration read.");

    let config = toml::Value::Table(parsed_toml);
    match toml::decode(config) {
        Some(c) => Ok(c),
        None => Err("error while deserializing config".to_string())
    }
}

#[cfg(test)]
mod tests {
    extern crate hyper;

    use std::thread;
    use super::{
        CanaryConfig, CanaryAlertConfig, CanaryLogConfig,
        CanaryCheck, CanaryTargetTypes, CanaryTarget,
        read_config, check_host
    };
    use hyper::server::{Server, Request, Response};

    #[test]
    fn it_reads_and_parses_a_config_file() {
        let expected = CanaryConfig {
            log: CanaryLogConfig {
                enabled: true,
                dir_path: "log".to_string(),
                file: false,
                stdout: true
            },
            alert: CanaryAlertConfig {
                enabled: true,
                alert_email: "rcanary.alert.inbox@gmail.com".to_string(),
                smtp_server: "smtp.googlemail.com".to_string(),
                smtp_username: "example@gmail.com".to_string(),
                smtp_password: "hunter2".to_string(),
                smtp_port: 587
            },
            server_listen_address: "127.0.0.1:8099".to_string(),
            targets: CanaryTargetTypes {
                http: vec!(
                    CanaryTarget {
                        name: "Invalid".to_string(),
                        host: "Hello, world!".to_string(),
                        interval_s: 60,
                        alert: false
                    },
                    CanaryTarget {
                        name: "404".to_string(),
                        host: "http://www.google.com/404".to_string(),
                        interval_s: 5,
                        alert: false
                    },
                    CanaryTarget {
                        name: "Google".to_string(),
                        host: "https://www.google.com".to_string(),
                        interval_s: 5,
                        alert: false
                    },
                )
            }
        };

        let actual = read_config("tests/fixtures/config.toml").unwrap();

        assert_eq!(expected, actual);
    }

    #[test]
    fn it_checks_invalid_target_hosts() {
        let target = CanaryTarget {
            name: "foo".to_string(),
            host: "invalid".to_string(),
            interval_s: 1,
            alert: false
        };

        let actual = check_host(&target);

        let expected = CanaryCheck {
            target: target.clone(),
            time: actual.time.clone(),
            info: "unknown".to_string(),
            status_code: "failed to poll server: relative URL without a base".to_string(),
            alert: false,
            need_to_alert: true
        };

        assert_eq!(expected, actual);
    }

    #[test]
    fn it_checks_valid_target_hosts() {
        thread::spawn(move || {
            Server::http("0.0.0.0:56473").unwrap().handle(move |_req: Request, res: Response| {
                res.send(b"I love BGP").unwrap();
            }).unwrap();
        });

        let ok_target = CanaryTarget {
            name: "foo".to_string(),
            host: "http://0.0.0.0:56473".to_string(),
            interval_s: 1,
            alert: false
        };

        let ok_actual = check_host(&ok_target);

        let ok_expected = CanaryCheck {
            target: ok_target.clone(),
            time: ok_actual.time.clone(),
            info: "okay".to_string(),
            status_code: "200 OK".to_string(),
            alert: false,
            need_to_alert: false
        };

        assert_eq!(ok_expected, ok_actual);
    }
}
