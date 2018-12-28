use librcanary::{CanaryCheck, CanaryTargetTypes};

pub mod prometheus;

pub trait Metrics {
    fn new(result: &CanaryTargetTypes) -> Self;
    fn update(&self, target_name: &str, result: &CanaryCheck) -> Result<(), String>;
    fn print(&self) -> Result<String, String>;
}
