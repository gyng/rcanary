use librcanary::CanaryCheck;
use librcanary::CanaryTargetTypes;
use metrics::Metrics;
use std::collections::HashMap;

use prometheus::{Encoder, Gauge, Registry, TextEncoder};

#[derive(Clone)]
pub struct PrometheusMetrics {
    pub gauges: HashMap<String, Gauge>,
    pub registry: Registry,
}

impl Metrics for PrometheusMetrics {
    fn new(targets: &CanaryTargetTypes) -> PrometheusMetrics {
        let registry = Registry::new();
        let mut gauges: HashMap<String, Gauge> = HashMap::new();

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
            gauges.insert(status_tag, status_gauge);

            let latency_tag = format!("{}_latency_ms", &tag);
            let latency_opts = opts!(latency_tag.clone(), format!("latency for {}", &tag));
            let latency_gauge = register_gauge!(latency_opts)
                .expect(&format!("failed to create latency gauge for {}", &tag));
            registry
                .register(Box::new(latency_gauge.clone()))
                .expect(&format!("failed to register gauge: {}", &tag));
            gauges.insert(latency_tag, latency_gauge);
        }

        PrometheusMetrics {
            gauges: gauges,
            registry: registry,
        }
    }

    fn update(&self, tag: &str, result: &CanaryCheck) -> Result<(), String> {
        println!("{:?}", &result);

        let status_gauge = self.gauges.get(&format!("{}_status", tag));
        if let Some(gauge) = status_gauge {
            gauge.set(result.status_code.parse::<f64>().unwrap_or(-1.0f64));
        }

        let latency_gauge = self.gauges.get(&format!("{}_latency", tag));
        if let Some(gauge) = latency_gauge {
            gauge.set(result.latency_ms as f64);
        }

        if status_gauge.is_some() && latency_gauge.is_some() {
            Ok(())
        } else {
            Err("could not update a gauge".to_string())
        }
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
