use librcanary::CanaryCheck;

pub mod alert;
pub mod email;

pub trait Alerter {
    fn alert(&self, result: &CanaryCheck) -> Result<(), String>;
}
