extern crate hyper;
extern crate rustc_serialize;
extern crate time;
extern crate toml;
extern crate ws;

mod ws_handler;

use std::env;
use std::fs::File;
use std::fs;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::result::Result;
use std::sync::mpsc;
use std::thread;
use std::time::Duration;

use rustc_serialize::json;

#[derive(RustcDecodable, RustcEncodable, Eq, PartialEq, Clone, Debug)]
pub struct CanaryConfig {
    target: CanaryTargetTypes,
    server_listen_address: String
}

#[derive(RustcDecodable, RustcEncodable, Eq, PartialEq, Clone, Debug)]
struct CanaryTargetTypes {
    http: Vec<CanaryTarget>
}

#[derive(RustcDecodable, RustcEncodable, Eq, PartialEq, Clone, Debug)]
struct CanaryTarget {
    name: String,
    host: String,
    interval_s: u64
}

#[derive(RustcEncodable, Eq, PartialEq, Clone, Debug)]
pub struct CanaryCheck {
    target: CanaryTarget,
    info: String,
    status_code: String,
    time: String
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

    for http_target in config.clone().target.http {
        let child_poll_tx = poll_tx.clone();

        thread::spawn(move || {
            loop {
                let _ = child_poll_tx.send(check_host(&http_target));
                thread::sleep(Duration::new(http_target.interval_s, 0));
            }
        });
    }

    // Start up websocket server
    let me = ws::WebSocket::new(ws_handler::ClientFactory { config: config.clone() }).unwrap();
    let broadcaster = me.broadcaster();

    thread::spawn(move || {
        me.listen(config.server_listen_address.as_str()).unwrap();
    });

    // Broadcast to all clients
    loop {
        let result = poll_rx.recv().unwrap();
        log_result(&result);
        // println!("{:#?}", result);
        let _ = broadcaster.send(json::encode(&result).unwrap());
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
            status_code: format!("failed to poll server: {}", err)
        }
    }

    // Should never panic on unwrap
    let response = response_raw.unwrap();
    let info = if response.status.is_success() {
        "okay".to_string()
    } else {
        "fire".to_string()
    };

    CanaryCheck {
        target: target.clone(),
        time: format!("{}", time::now_utc().rfc3339()),
        info: info,
        status_code: format!("{}", response.status)
    }
}

fn log_result(result: &CanaryCheck) {
    let log_dir = "log";
    if !Path::new(log_dir).exists() {
        fs::create_dir(log_dir)
            .expect("failed to create log directory");
    }

    // println!("logging! {:?}", result);
    let path = PathBuf::from("log/log.txt");
    let mut f = File::create(path).expect("failed ot open log file for writing");
    let _ = f.write_all(format!("{:?}", result).as_bytes());
}

fn read_config(path: &str) -> Result<CanaryConfig, String> {
    println!("Reading configuration from `{}`", path);

    let mut file = match File::open(&path) {
        Ok(f) => f,
        Err(err) => return Err(format!("Failed to read file {}", err))
    };

    let mut config_toml = String::new();
    if let Err(err) = file.read_to_string(&mut config_toml) {
        return Err(format!("Error reading config: {}", err))
    }

    let parsed_toml = toml::Parser::new(&config_toml).parse().unwrap();
        // .unwrap_or_else(|err| panic!("Error parsing config file: {}", err));

    let config = toml::Value::Table(parsed_toml);
    match toml::decode(config) {
        Some(c) => Ok(c),
        None => Err("Error while deserializing config".to_string())
    }
}

#[cfg(test)]
mod tests {
    extern crate hyper;

    use std::thread;
    use super::{CanaryConfig, CanaryCheck, CanaryTargetTypes, CanaryTarget, read_config, check_host};
    use hyper::server::{Server, Request, Response};

    #[test]
    fn it_reads_and_parses_a_config_file() {
        let expected = CanaryConfig {
            server_listen_address: "127.0.0.1:8099".to_string(),
            target: CanaryTargetTypes {
                http: vec!(
                    CanaryTarget {
                        name: "Invalid".to_string(),
                        host: "Hello, world!".to_string(),
                        interval_s: 60
                    },
                    CanaryTarget {
                        name: "404".to_string(),
                        host: "http://www.google.com/404".to_string(),
                        interval_s: 5
                    },
                    CanaryTarget {
                        name: "Google".to_string(),
                        host: "https://www.google.com".to_string(),
                        interval_s: 5
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
            interval_s: 1
        };

        let actual = check_host(&target);

        let expected = CanaryCheck {
            target: target.clone(),
            time: actual.time.clone(),
            info: "unknown".to_string(),
            status_code: "failed to poll server: relative URL without a base".to_string()
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
            interval_s: 1
        };

        let ok_actual = check_host(&ok_target);

        let ok_expected = CanaryCheck {
            target: ok_target.clone(),
            time: ok_actual.time.clone(),
            info: "okay".to_string(),
            status_code: "200 OK".to_string()
        };

        assert_eq!(ok_expected, ok_actual);
    }
}