use std::any::Any;
use std::io;
use std::net::{IpAddr, ToSocketAddrs};
use std::pin::Pin;

use futures::compat::{Compat, Future01CompatExt};
use futures::future::{self, join_all, Future, FutureExt, Ready};
use hyper::client::connect::dns::Name;
use hyper::Uri;

use super::{Check, CheckFailure, CheckResult};

const CHECK_NAME: &str = "http";

#[derive(Clone, Debug)]
pub struct HttpTarget {
    pub url: Uri,
}

pub struct HttpCheck;

fn invalid_target_missing_value(
    url: &str,
    field: &str,
) -> Pin<Box<dyn Future<Output = Result<CheckResult, io::Error>> + Send + 'static>> {
    let msg = format!("invalid target: URL {} is missing a {}", url, field);
    debug!("invalid_target_missing_value: {}", msg);
    future::err(io::Error::new(io::ErrorKind::Other, msg)).boxed()
}

fn invalid_target_invalid_value(
    url: &str,
    field: &str,
    value: &str,
) -> Pin<Box<dyn Future<Output = Result<CheckResult, io::Error>> + Send + 'static>> {
    let msg = format!(
        "invalid target: URL {} has invalid {}: {}",
        url, field, value
    );
    debug!("invalid_target_invalid_value: {}", msg);
    future::err(io::Error::new(io::ErrorKind::Other, msg)).boxed()
}

impl Check for HttpCheck {
    type Target = HttpTarget;
    type Future = Pin<Box<dyn Future<Output = Result<CheckResult, io::Error>> + Send>>;

    fn check(&self, target: Self::Target) -> Self::Future {
        use http::uri::Parts;
        let uri_string = target.url.to_string();

        let parts: Parts = target.url.clone().into();
        let scheme = match parts.scheme {
            Some(s) => s,
            None => return invalid_target_missing_value(&uri_string, "scheme"),
        };
        let authority = match parts.authority {
            Some(a) => a,
            None => return invalid_target_missing_value(&uri_string, "authority"),
        };

        let mut port;
        match scheme.as_str() {
            "http" => port = 80,
            "https" => port = 443,
            s => return invalid_target_invalid_value(&uri_string, "scheme", s),
        };
        if let Some(p) = authority.port_part() {
            port = p.as_u16()
        }

        let netloc = (authority.host().to_string(), port);
        check_impl(netloc, target.url.clone()).boxed()
    }
}

async fn check_impl(netloc: (String, u16), url: Uri) -> Result<CheckResult, io::Error> {
    // Do DNS lookup.  Check fails if DNS fails.
    // FIXME/XXX: this is blocking.
    let netloc: (&str, u16) = (&netloc.0, netloc.1);
    let addrs: Vec<_> = netloc.to_socket_addrs()?.map(|s| s.ip()).collect();
    debug!("check_impl: addrs={:?}", addrs);

    let results: Vec<CheckResult> = join_all(
        addrs
            .into_iter()
            .map(|s| connect_and_request(s, url.clone())),
    )
    .await;

    Ok(CheckResult::merge(results.into_iter()))
}

#[derive(Clone)]
struct StaticResolverSingle {
    ip_addr: IpAddr,
}

impl hyper::client::connect::dns::Resolve for StaticResolverSingle {
    type Addrs = std::vec::IntoIter<IpAddr>;
    type Future = Compat<Ready<Result<Self::Addrs, io::Error>>>;

    fn resolve(&self, _name: Name) -> Self::Future {
        Compat::new(future::ok(vec![self.ip_addr].into_iter()))
    }
}

async fn connect_and_request(ip_addr: IpAddr, target: Uri) -> CheckResult {
    use hyper::client::{Client, HttpConnector};

    debug!("connect_and_request: connecting to {}", ip_addr);
    let connector = HttpConnector::new_with_resolver(StaticResolverSingle { ip_addr });

    let client: Client<_> = Client::builder().build(connector);
    let resp: hyper::Response<_> = match client.get(target).compat().await {
        Ok(r) => r,
        Err(err) => {
            debug!("connect_and_request: got err {}", err);
            let err = CheckFailure {
                check_name: CHECK_NAME,
                error: Box::new(err),
            };
            return CheckResult::fail(err);
        }
    };

    let status = resp.status();
    debug!("connect_and_request: got response with status {}", status);
    if status.is_server_error() || status.is_client_error() {
        return CheckResult::fail(CheckFailure {
            check_name: CHECK_NAME,
            error: Box::new(HttpBadStatus {
                status: status.as_u16(),
            }),
        });
    }

    CheckResult::succeed()
}

struct HttpBadStatus {
    status: u16,
}
