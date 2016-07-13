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

#[derive(RustcDecodable, Eq, PartialEq, Clone, Debug)]
struct CanaryConfig {
    target: Vec<CanaryTarget>,
    server_listen_address: String
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

    for target in config.clone().target {
        let child_poll_tx = poll_tx.clone();

        thread::spawn(move || {
            loop {
                let _ = child_poll_tx.send(check_host(&target));
                thread::sleep(Duration::new(target.interval_s, 0));
            }
        });
    }

    // Start up websocket server
    let me = ws::WebSocket::new(ws_handler::ClientFactory).unwrap();
    let broadcaster = me.broadcaster();

    thread::spawn(move || {
        me.listen(config.server_listen_address.as_str()).unwrap();
    });

    // Broadcast to all clients
    loop {
        let result = poll_rx.recv().unwrap();
        log_result(&result);
        println!("{:#?}", result);
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
            info: format!("failed to poll server: {}", err),
            status_code: "null".to_string()
        }
    }

    // Should never panic on unwrap
    let response = response_raw.unwrap();
    let info = if response.status.is_success() {
        "okay".to_string()
    } else {
        "no idea".to_string()
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

    println!("logging! {:?}", result);
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
    use super::{CanaryConfig, CanaryTarget, read_config};

    #[test]
    fn it_reads_and_parses_a_config_file() {
        let expected = CanaryConfig {
            server_listen_address: "127.0.0.1:8099".to_string(),
            target: vec!(
                CanaryTarget {
                    name: "Hello,".to_string(),
                    host: "world!".to_string(),
                    interval_s: 60
                },
                CanaryTarget {
                    name: "Google".to_string(),
                    host: "http://www.google.com".to_string(),
                    interval_s: 5
                },
            )
        };

        let actual = read_config("test/fixtures/config.toml").unwrap();

        assert_eq!(expected, actual);
    }
}