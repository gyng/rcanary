extern crate hyper;
extern crate rustc_serialize;
extern crate toml;

use std::env;
use std::fs::File;
use std::io::Read;
use std::result::Result;
use std::sync::mpsc;
use std::thread;
use std::time::Duration;

use hyper::Client;

#[derive(RustcDecodable, Eq, PartialEq, Clone, Debug)]
struct CanaryConfig {
    target: Vec<CanaryTarget>
}

#[derive(RustcDecodable, Eq, PartialEq, Clone, Debug)]
struct CanaryTarget {
    name: String,
    host: String,
    interval_s: u64
}

fn main() {
    let config_path = match env::args().nth(1) {
        Some(c) => c,
        None => panic!("no configuration file supplied as the first argument")
    };

    let config = match read_config(&config_path) {
        Ok(c) => c,
        Err(err) => panic!("{} -- Invalid configuration file {}", err, config_path.clone())
    };

    let (tx, rx) = mpsc::channel();

    for target in config.target {
        let child_tx = tx.clone();

        thread::spawn(move || {
            loop {
                let _ = child_tx.send(check_host(target.clone()));
                thread::sleep(Duration::new(target.interval_s, 0));
            }
        });
    }

    loop {
        let result = rx.recv().unwrap();
        log_result(result);
    }
}

fn check_host(config: CanaryTarget) -> Result<(), String> {
    println!("checking {:#?}", config);

    let response = Client::new().get("http://bgp-ci.ida-gds-demo.com").send();

    return match response {
        Ok(_) => Ok(()),
        Err(_err) => Err("no go".to_owned())
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
            target: vec!(
                CanaryTarget {
                    name: "Hello,".to_owned(),
                    host: "world!".to_owned(),
                    interval_s: 60
                },
                CanaryTarget {
                    name: "foo".to_owned(),
                    host: "bar".to_owned(),
                    interval_s: 30
                },
            )
        };

        let actual = read_config("test/fixtures/config.toml").unwrap();

        assert_eq!(expected, actual);
    }
}