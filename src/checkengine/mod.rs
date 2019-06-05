use std::fmt;
use std::io;
use std::net::IpAddr;
use std::time::Instant;

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

#[derive(Debug, PartialEq)]
pub enum CheckStatus {
    Alive,
    Degraded,
    Failed,
}

pub struct CheckTimeSpan {
    name: &'static str,
    started_at: Instant,
    ended_at: Instant,
}

impl fmt::Debug for CheckTimeSpan {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.debug_struct("CheckTimeSpan")
            .field("name", &self.name)
            .field("started_at", &self.started_at)
            .field("ended_at", &self.ended_at)
            .field(
                "length_hack",
                &format!("{:?}", self.ended_at - self.started_at),
            )
            .finish()
    }
}

#[derive(Debug)]
pub struct CheckResultElement {
    target: IpAddr,
    status: CheckStatus,
    err_msg: Option<String>,
    timeline: Vec<CheckTimeSpan>,
}

#[derive(Debug)]
pub struct CheckResult {
    name: &'static str,
    elements: Vec<CheckResultElement>,
}

impl CheckResult {
    pub fn new(name: &'static str, e: CheckResultElement) -> CheckResult {
        CheckResult {
            name,
            elements: vec![e],
        }
    }

    pub fn status(&self) -> CheckStatus {
        for e in &self.elements {
            if e.status == CheckStatus::Failed {
                return CheckStatus::Failed;
            }
        }
        for e in &self.elements {
            if e.status == CheckStatus::Degraded {
                return CheckStatus::Degraded;
            }
        }
        CheckStatus::Alive
    }

    pub fn merge<I>(mut from: I) -> CheckResult
    where
        I: Iterator<Item = CheckResult>,
    {
        let mut out = from.next().unwrap();
        for i in from {
            out.elements.extend(i.elements.into_iter());
        }
        out
    }
}
