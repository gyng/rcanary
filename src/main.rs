extern crate docopt;
extern crate env_logger;
#[macro_use]
extern crate lazy_static;
extern crate lettre;
#[macro_use]
extern crate log;
extern crate reqwest;
#[macro_use]
extern crate serde_derive;
extern crate librcanary;
#[macro_use]
extern crate prometheus;
extern crate serde;
extern crate serde_json;
extern crate time;
extern crate toml;
extern crate ws;

mod alerter;
mod metrics;
mod ws_handler;

use metrics::prometheus::PrometheusMetrics;
use metrics::Metrics;
use std::sync::Arc;

use std::collections::HashMap;
use std::env;
use std::error::Error;
use std::fs::File;
use std::io::Read;
use std::net::SocketAddr;
use std::sync::mpsc;
use std::thread;
use std::time::{Duration, Instant};

use docopt::Docopt;
use librcanary::*;
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
            panic!(
                "[status.startup] failed to read configuration file {}: {}",
                &args.arg_configuration_file, err
            );
        })
        .unwrap();

    let metrics_handler = if config.metrics.is_some() && config.clone().metrics.unwrap().enabled {
        // TODO: handle multiple types of metrics handlers
        Arc::new(Some(metrics::prometheus::PrometheusMetrics::new(
            &config.targets,
        )))
    } else {
        Arc::new(None)
    };

    // Setup map to save results
    let mut last_statuses = HashMap::new();

    // Start polling
    let (poll_tx, poll_rx) = mpsc::channel();

    for http_target in config.clone().targets.http {
        let child_poll_tx = poll_tx.clone();
        let child_metrics = metrics_handler.clone();

        thread::spawn(move || loop {
            let result = check_host(&http_target);

            if let Ok(Some(handler)) = Arc::try_unwrap(child_metrics.clone()) {
                // It's okay if metrics fail to update (maybe?)
                let _ = handler.update(&http_target.tag_metric.clone().unwrap(), &result);
            }

            let _ = child_poll_tx.send(result);
            thread::sleep(Duration::new(http_target.interval_s, 0));
        });
    }

    if let Some(health_check_config) = config.clone().health_check {
        if health_check_config.enabled {
            start_healthcheck_server(&health_check_config.address);
        }
    }

    if let Some(metrics_config) = config.clone().metrics {
        if metrics_config.enabled {
            start_metrics_server(&metrics_config.address, metrics_handler);
        }
    }

    // Start up websocket server
    info!("[status.startup] starting websocker server...");
    let me = ws::WebSocket::new(ws_handler::ClientFactory {
        config: config.clone(),
    })
    .unwrap_or_else(|err| {
        panic!("[status.startup] failed to start websocket server {}", err);
    });
    info!("[status.startup] started websocker server.");
    let broadcaster = me.broadcaster();
    let config_clone = config.clone();
    thread::spawn(move || {
        me.listen(&*config_clone.server_listen_address)
            .unwrap_or_else(|err| {
                panic!(
                    "[status.startup] failed to start websocket listener {}",
                    err
                );
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

        let is_spam = alerter::alert::check_spam(&last_statuses, &result);
        let is_fixed = alerter::alert::check_fixed(&last_statuses, &result);
        last_statuses.insert(result.target.clone(), result.status.clone());

        if config.alert.enabled && result.alert && (is_fixed || result.need_to_alert && !is_spam) {
            let child_config = config.clone();
            let child_result = result.clone();
            thread::spawn(move || alerter::alert::send_alert(&child_config, &child_result));
        }

        if let Ok(json) = serde_json::to_string(&result) {
            let _ = broadcaster.send(json);
        } else {
            panic!(
                "[status.startup] failed to encode result into json {:?}",
                &result
            );
        }
    }
}

fn check_host(target: &CanaryTarget) -> CanaryCheck {
    let mut headers = Headers::new();
    headers.set(UserAgent::new("rcanary/0.5.0"));

    if let Some(ref a) = target.basic_auth {
        headers.set(Authorization(Basic {
            username: a.username.clone(),
            password: a.password.clone(),
        }))
    };

    let latency_timer = Instant::now();
    let request = reqwest::Client::new().and_then(|r| r.get(&target.host));

    let (need_to_alert, status, status_code) = match request {
        Ok(mut request) => match request.headers(headers).send() {
            Ok(ref r) if r.status().is_success() => (false, Status::Okay, r.status().to_string()),
            Ok(ref r) => (true, Status::Fire, r.status().to_string()),
            Err(err) => (
                true,
                Status::Unknown,
                format!("failed to poll server: {}", err),
            ),
        },
        Err(err) => (false, Status::Unknown, format!("bad URL: {}", err)),
    };

    let latency = latency_timer.elapsed();
    let nanos = latency.subsec_nanos() as u64;
    let latency_ms = (1000 * 1000 * 1000 * latency.as_secs() + nanos) / (1000 * 1000);

    CanaryCheck {
        target: target.clone(),
        time: format!("{}", time::now_utc().rfc3339()),
        status: status,
        status_code: status_code,
        status_reason: "unimplemented".to_string(),
        latency_ms,
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

fn start_metrics_server(bind_to: &str, metrics_handler: Arc<Option<PrometheusMetrics>>) {
    let addr: SocketAddr = bind_to.parse().unwrap_or_else(|err| {
        panic!("[status.startup] failed to start metrics endpoint: {}", err);
    });

    info!("[status.startup] starting metrics server at {}...", &addr);

    thread::spawn(move || {
        let child_metrics = metrics_handler.clone();

        rouille::start_server(addr, move |_req| {
            let child_arc = child_metrics.clone();
            let body = if let Ok(Some(handler)) = Arc::try_unwrap(child_arc) {
                handler.print().unwrap()
            } else {
                "None".to_string()
            };

            rouille::Response::text(body)
        });
    });
}

fn start_healthcheck_server(bind_to: &str) {
    let addr: SocketAddr = bind_to.parse().unwrap_or_else(|err| {
        panic!(
            "[status.startup] failed to start health check endpoint: {}",
            err
        );
    });

    info!(
        "[status.startup] starting health check server at {}...",
        &addr
    );

    thread::spawn(move || {
        rouille::start_server(addr, move |_req| rouille::Response::text("OK"));
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{thread, time};

    fn sleep() {
        let delay = time::Duration::from_millis(250);
        thread::sleep(delay);
    }

    pub fn target() -> CanaryTarget {
        CanaryTarget {
            name: "foo".to_string(),
            host: "invalid".to_string(),
            tag: Some("tag".to_string()),
            tag_metric: None,
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
                email: Some(CanaryEmailAlertConfig {
                    alert_email: "rcanary.alert.inbox@gmail.com".to_string(),
                    smtp_server: "smtp.googlemail.com".to_string(),
                    smtp_username: "example@gmail.com".to_string(),
                    smtp_password: "hunter2".to_string(),
                    smtp_port: 587,
                }),
            },
            metrics: Some(CanaryMetricsConfig {
                enabled: false,
                address: "127.0.0.1:9809".to_string(),
            }),
            health_check: Some(CanaryHealthCheckConfig {
                enabled: true,
                address: "127.0.0.1:8100".to_string(),
            }),
            server_listen_address: "127.0.0.1:8099".to_string(),
            targets: CanaryTargetTypes {
                http: vec![
                    CanaryTarget {
                        name: "Invalid".to_string(),
                        host: "Hello, world!".to_string(),
                        tag: None,
                        tag_metric: Some("hello".to_string()),
                        interval_s: 60,
                        alert: false,
                        basic_auth: None,
                    },
                    CanaryTarget {
                        name: "404".to_string(),
                        host: "http://www.google.com/404".to_string(),
                        tag: Some("example-tag".to_string()),
                        tag_metric: Some("http_404".to_string()),
                        interval_s: 5,
                        alert: false,
                        basic_auth: None,
                    },
                    CanaryTarget {
                        name: "localhost:8080".to_string(),
                        host: "http://localhost:8080".to_string(),
                        tag: None,
                        tag_metric: Some("local_8080".to_string()),
                        interval_s: 5,
                        alert: false,
                        basic_auth: None,
                    },
                    CanaryTarget {
                        name: "Google".to_string(),
                        host: "https://www.google.com".to_string(),
                        tag: None,
                        tag_metric: Some("google".to_string()),
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
            alert: false,
            latency_ms: actual.latency_ms,
            need_to_alert: false,
            status_code: "bad URL: relative URL without a base".to_string(),
            status: Status::Unknown,
            status_reason: "unimplemented".to_string(),
            target: target(),
            time: actual.time.clone(),
        };

        assert_eq!(expected, actual);
    }

    #[test]
    fn it_checks_valid_target_hosts() {
        static TEXT: &'static str = "I love BGP";
        thread::spawn(move || {
            rouille::start_server("127.0.0.1:56473", move |_req| rouille::Response::text(TEXT));
        });
        sleep();

        let ok_target = CanaryTarget {
            name: "foo".to_string(),
            host: "http://127.0.0.1:56473".to_string(),
            tag: Some("bar".to_string()),
            tag_metric: None,
            interval_s: 1,
            alert: false,
            basic_auth: None,
        };

        let ok_actual = check_host(&ok_target);

        let ok_expected = CanaryCheck {
            alert: false,
            latency_ms: ok_actual.latency_ms,
            need_to_alert: false,
            status_code: "200 OK".to_string(),
            status: Status::Okay,
            status_reason: "unimplemented".to_string(),
            target: ok_target.clone(),
            time: ok_actual.time.clone(),
        };

        assert_eq!(ok_expected, ok_actual);
    }

    #[test]
    fn it_checks_valid_target_hosts_with_basic_auth() {
        thread::spawn(move || {
            rouille::start_server("127.0.0.1:56474", move |req| {
                assert_eq!(
                    req.header("Authorization").unwrap(),
                    "Basic QXp1cmVEaWFtb25kOmh1bnRlcjI=" // hunter2
                );
                rouille::Response::text("OK")
            });
        });
        sleep();

        let ok_target = CanaryTarget {
            name: "foo".to_string(),
            host: "http://127.0.0.1:56474".to_string(),
            tag: Some("bar".to_string()),
            tag_metric: None,
            interval_s: 1,
            alert: false,
            basic_auth: Some(Auth {
                username: "AzureDiamond".to_string(),
                password: Some("hunter2".to_string()),
            }),
        };

        let ok_actual = check_host(&ok_target);

        let ok_expected = CanaryCheck {
            alert: false,
            latency_ms: ok_actual.latency_ms,
            need_to_alert: false,
            status_code: "200 OK".to_string(),
            status: Status::Okay,
            status_reason: "unimplemented".to_string(),
            target: ok_target.clone(),
            time: ok_actual.time.clone(),
        };

        assert_eq!(ok_expected, ok_actual);
    }
}
