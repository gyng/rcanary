use std::sync::Mutex;
use std::collections::HashMap;
use librcanary::CanaryCheck;
use librcanary::CanaryTargetTypes;
use metrics::Metrics;

use prometheus::{Encoder, Gauge, Registry, TextEncoder};

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
        // let mut gauges: Map<String, Gauge> = Map::new();

        for target in targets.clone().http {
            // We want metrics setup failures to surface ASAP (on startup)
            let tag = target
                .tag_metric
                .expect(&format!("Missing tag_metric for {:?}", target.host));

            let status_tag = format!("{}_status", tag);
            let status_opts = opts!(status_tag.clone(), format!("status for {}", tag));
            let status_gauge = Gauge::with_opts(status_opts)
                .expect(&format!("failed to create status gauge for {}", tag));
            registry
                .register(Box::new(status_gauge.clone()))
                .expect(&format!("failed to register gauge: {}", tag));
            GAUGES.lock().unwrap().insert(status_tag, status_gauge);

            let latency_tag = format!("{}_latency_ms", &tag);
            let latency_opts = opts!(latency_tag.clone(), format!("latency for {}", &tag));
            let latency_gauge = register_gauge!(latency_opts)
                .expect(&format!("failed to create latency gauge for {}", &tag));
            registry
                .register(Box::new(latency_gauge.clone()))
                .expect(&format!("failed to register gauge: {}", &tag));
            GAUGES.lock().unwrap().insert(latency_tag, latency_gauge);
        }

        PrometheusMetrics {
            registry: registry,
        }
    }

    fn update(&self, tag: &str, result: &CanaryCheck) -> Result<(), String> {
        println!("{:?}", &result);

        let gauges = GAUGES.lock().unwrap();

        let status_gauge = gauges.get(&format!("{}_status", tag));
        if let Some(gauge) = status_gauge {
            gauge.set(result.status_code.parse::<f64>().unwrap_or(-1.0f64));
        }

        let latency_gauge = gauges.get(&format!("{}_latency", tag));
        if let Some(gauge) = latency_gauge {
            gauge.set(result.latency_ms as f64);
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
                .unwrap_or("failed to create string from buffer".to_string()))
        } else {
            Err("failed to encode printable output".to_string())
        }
    }
}
