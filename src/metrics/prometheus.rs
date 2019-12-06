use std::collections::HashMap;
use std::sync::Mutex;

use lazy_static::lazy_static;
use prometheus::{opts, Encoder, Gauge, Registry, TextEncoder};

use librcanary::CanaryCheck;
use librcanary::CanaryTargetTypes;

use super::Metrics;

lazy_static! {
    static ref GAUGES: Mutex<HashMap<String, Gauge>> = {
        let m = HashMap::new();
        Mutex::new(m)
    };
}

#[derive(Clone)]
pub struct PrometheusMetrics {
    pub registry: Registry,
}

impl Metrics for PrometheusMetrics {
    fn new(targets: &CanaryTargetTypes) -> PrometheusMetrics {
        let registry = Registry::new();

        for target in targets.clone().http {
            // We want metrics setup failures to surface ASAP (on startup)
            #[allow(clippy::expect_fun_call)] // borrow-ck issue, might go away with NLL?
            let tag = target
                .tag_metric
                .expect(&format!("Missing tag_metric for {:?}", target.host));

            let status_tag = format!("{}_status", tag);
            let status_opts = opts!(status_tag.clone(), format!("status for {}", tag));
            let status_gauge = Gauge::with_opts(status_opts)
                .unwrap_or_else(|_| panic!("failed to create status gauge for {}", tag));
            registry
                .register(Box::new(status_gauge.clone()))
                .unwrap_or_else(|_| panic!("failed to register gauge: {}", tag));
            GAUGES
                .lock()
                .expect("GAUGES mutex is poisoned")
                .insert(status_tag, status_gauge);

            let latency_tag = format!("{}_latency_ms", &tag);
            let latency_opts = opts!(latency_tag.clone(), format!("latency for {}", &tag));
            let latency_gauge = register_gauge!(latency_opts)
                .unwrap_or_else(|_| panic!("failed to create latency gauge for {}", &tag));
            registry
                .register(Box::new(latency_gauge.clone()))
                .unwrap_or_else(|_| panic!("failed to register gauge: {}", &tag));
            GAUGES
                .lock()
                .expect("GAUGES mutex is poisoned")
                .insert(latency_tag, latency_gauge);
        }

        PrometheusMetrics { registry }
    }

    fn update(&self, tag: &str, result: &CanaryCheck) -> Result<(), String> {
        if let Ok(gauges) = GAUGES.lock() {
            let status_gauge = gauges.get(&format!("{}_status", tag));
            if let Some(gauge) = status_gauge {
                gauge.set(result.status_code.parse::<f64>().unwrap_or(-1.0f64));
            }

            let latency_gauge = gauges.get(&format!("{}_latency_ms", tag));
            if let Some(gauge) = latency_gauge {
                gauge.set(result.latency_ms as f64);
            }
        } else {
            return Err("Failed to update gauges: GAUGES mutex is poisoned".to_string());
        }

        Ok(())
    }

    fn print(&self) -> Result<String, String> {
        let mut buffer = vec![];
        let encoder = TextEncoder::new();
        let metric_families = self.registry.gather();
        let encoded_result = encoder.encode(&metric_families, &mut buffer);

        if encoded_result.is_ok() {
            // Should be OK
            Ok(String::from_utf8(buffer)
                .unwrap_or_else(|_| "failed to create string from buffer".to_string()))
        } else {
            Err("failed to encode printable output".to_string())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use librcanary::*;

    fn test_targets() -> CanaryTargetTypes {
        CanaryTargetTypes {
            http: vec![CanaryTarget {
                alert: false,
                basic_auth: None,
                host: "127.0.0.1".to_string(),
                interval_s: 10,
                name: "foo".to_string(),
                tag_metric: Some("footag".to_string()),
                tag: None,
            }],
        }
    }

    fn ok_result() -> CanaryCheck {
        let target = test_targets().http.get(0).unwrap().clone();
        CanaryCheck {
            alert: false,
            latency_ms: 1234,
            need_to_alert: false,
            status_code: "200".to_string(),
            status_reason: "foobar".to_string(),
            status: Status::Okay,
            target,
            time: "1234".to_string(),
        }
    }

    #[test]
    fn it_creates_updates_and_prints_the_metrics_registry() {
        let targets = test_targets();
        let metrics: PrometheusMetrics = Metrics::new(&targets);

        let ok = ok_result();
        metrics
            .update("footag", &ok)
            .expect("failed to update metrics");

        let expected = "# HELP footag_latency_ms latency for footag\n\
                        # TYPE footag_latency_ms gauge\n\
                        footag_latency_ms 1234\n\
                        # HELP footag_status status for footag\n\
                        # TYPE footag_status gauge\n\
                        footag_status 200\n\
                        ";

        assert_eq!(metrics.print().unwrap(), expected);
    }
}
