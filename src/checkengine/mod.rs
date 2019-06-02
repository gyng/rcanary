use std::any::Any;
use std::io;

use futures::future::Future;

mod http;
mod tcp;

pub use self::http::{HttpCheck, HttpTarget};
// pub use self::tcp::TcpConnectCheck;

pub trait Check {
    type Target: Clone;
    type Future: Future<Output = Result<CheckResult, io::Error>>;

    fn check(&self, target: Self::Target) -> Self::Future;
}

#[derive(Debug)]
pub struct CheckResult {
    checks_passed: i64,
    checks_total: i64,
    failures: Vec<CheckFailure>,
}

impl CheckResult {
    fn zero() -> CheckResult {
        CheckResult {
            checks_passed: 0,
            checks_total: 0,
            failures: Vec::new(),
        }
    }

    pub fn merge<I>(from: I) -> CheckResult
    where
        I: Iterator<Item = CheckResult>,
    {
        let mut out = CheckResult::zero();
        for i in from {
            out.checks_passed += i.checks_passed;
            out.checks_total += i.checks_total;
            out.failures.extend(i.failures.into_iter());
        }
        out
    }

    pub fn succeed() -> CheckResult {
        CheckResult {
            checks_passed: 1,
            checks_total: 1,
            failures: vec![],
        }
    }

    pub fn fail(failure: CheckFailure) -> CheckResult {
        CheckResult {
            checks_passed: 0,
            checks_total: 1,
            failures: vec![failure],
        }
    }
}

#[derive(Debug)]
pub struct CheckFailure {
    check_name: &'static str,
    error: Box<dyn Any + Send + 'static>,
}
