extern crate hyper;
extern crate rustc_serialize;
extern crate toml;
extern crate ws;

mod ws_handler;

use std::env;
use std::fs::File;
use std::io::Read;
use std::result::Result;
use std::sync::mpsc;
use std::thread;
use std::time::Duration;

#[derive(RustcDecodable, Eq, PartialEq, Clone, Debug)]
struct CanaryConfig {
    target: Vec<CanaryTarget>,
    server_listen_address: String
}

#[derive(RustcDecodable, Eq, PartialEq, Clone, Debug)]
struct CanaryTarget {
    name: String,
    host: String,
    interval_s: u64
}

#[derive(Eq, PartialEq, Clone, Debug)]
pub struct CanaryEvent {
    payload: String
}

fn main() {
    // Read config
    let config_path = match env::args().nth(1) {
        Some(c) => c,
        None => panic!("no configuration file supplied as the first argument")
    };

    let config = match read_config(&config_path) {
        Ok(c) => c,
        Err(err) => panic!("{} -- Invalid configuration file {}", err, config_path.clone())
    };

    // Start polling
    let (poll_tx, poll_rx) = mpsc::channel();

    for target in config.clone().target {
        let child_poll_tx = poll_tx.clone();

        thread::spawn(move || {
            loop {
                let _ = child_poll_tx.send(check_host(target.clone()));
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
        log_result(result);
        let _ = broadcaster.send("lol");
    }
}

fn check_host(_config: CanaryTarget) -> Result<(), String> {
    let response = hyper::Client::new().get("http://bgp-ci.ida-gds-demo.com").send();

    return match response {
        Ok(r) => {
            if r.status == hyper::status::StatusCode::Ok {
                Ok(())
            } else {
                Err(format!("bad status code: {}", r.status))
            }
        },
        Err(err) => Err(format!("failed to poll server: {}", err))
    }
}

fn log_result(result: Result<(), String>) {
    println!("logging! {:?}", result.unwrap());
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
        None => Err("Error while deserializing config".to_owned())
    }
}

#[cfg(test)]
mod tests {
    use super::{CanaryConfig, CanaryTarget, read_config};

    #[test]
    fn it_reads_and_parses_a_config_file() {
        let expected = CanaryConfig {
            server_listen_address: "127.0.0.1:8099".to_owned(),
            target: vec!(
                CanaryTarget {
                    name: "Hello,".to_owned(),
                    host: "world!".to_owned(),
                    interval_s: 60
                },
                CanaryTarget {
                    name: "foo".to_owned(),
                    host: "bar".to_owned(),
                    interval_s: 5
                },
            )
        };

        let actual = read_config("test/fixtures/config.toml").unwrap();

        assert_eq!(expected, actual);
    }
}