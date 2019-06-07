use std::io;
use std::net::{IpAddr, ToSocketAddrs};
use std::pin::Pin;
use std::time::{Duration, Instant};

use futures::compat::Future01CompatExt;
use futures::future::{self, join_all, Future, FutureExt};

use hyper::header::HeaderName;
use hyper::Uri;
use hyper_tls::HttpsConnector;
use native_tls::{self, TlsConnector};

use super::{Check, CheckResult, CheckResultElement, CheckStatus, CheckTimeSpan};

mod hyper_helpers;
use self::hyper_helpers::{HttpsConnectorWrapped, StaticResolverSingle};

const CHECK_NAME: &str = "http";

#[derive(Clone, Debug)]
pub struct HttpTarget {
    pub url: Uri,
    pub extra_headers: Vec<(HeaderName, String)>,
}

#[derive(Clone, Debug)]
pub struct HttpCheck {
    pub latency_requirement: Duration,
    pub allow_client_error: bool,
}

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
        check_impl(self.clone(), netloc, target.clone()).boxed()
    }
}

async fn check_impl(
    check: HttpCheck,
    netloc: (String, u16),
    target: HttpTarget,
) -> Result<CheckResult, io::Error> {
    // Do DNS lookup.  Check fails if DNS fails.
    // FIXME/XXX: this is blocking.
    let netloc: (&str, u16) = (&netloc.0, netloc.1);
    let addrs: Vec<_> = netloc.to_socket_addrs()?.map(|s| s.ip()).collect();
    debug!("check_impl: addrs={:?}", addrs);

    let results: Vec<CheckResult> = join_all(
        addrs
            .into_iter()
            .map(|s| connect_and_request(&check, s, target.clone())),
    )
    .await;

    Ok(CheckResult::merge(results.into_iter()))
}

async fn connect_and_request(
    check: &HttpCheck,
    ip_addr: IpAddr,
    target: HttpTarget,
) -> CheckResult {
    use hyper::body::Body;
    use hyper::client::{Client, HttpConnector};
    use hyper::header::USER_AGENT;
    use hyper::Request;

    let mut http_connector = HttpConnector::new_with_resolver(StaticResolverSingle::new(ip_addr));
    http_connector.enforce_http(false);

    let tls_connector = TlsConnector::builder().build().unwrap();
    let connector = HttpsConnector::from((http_connector, tls_connector));
    let connector = HttpsConnectorWrapped::from(connector);
    let conn_summary_handle = connector.summary_handle();

    let user_agent = format!("rcanary/{}", crate::CARGO_PKG_VERSION);

    let mut request = Request::get(target.url);
    request.header(USER_AGENT, user_agent);
    for (k, v) in target.extra_headers {
        request.header(k, v);
    }
    let request = request.body(Body::empty()).unwrap();

    let client: Client<_> = Client::builder().build(connector);
    let resp: hyper::Response<_> = match client.request(request).compat().await {
        Ok(r) => r,
        Err(err) => {
            let mut timeline = Vec::new();
            if let Some(s) = conn_summary_handle.summary() {
                if let Some(e) = s.connected_time() {
                    timeline.push(CheckTimeSpan {
                        name: "tls-handshake",
                        started_at: s.start_time(),
                        ended_at: e,
                    });
                }
            }

            return CheckResult::new(
                CHECK_NAME,
                CheckResultElement {
                    target: ip_addr,
                    check_status: CheckStatus::Failed,
                    status_code: 0,
                    err_msg: Some(format!("hyper error: {}", err)),
                    timeline,
                },
            );
        }
    };

    let finish_time = Instant::now();
    let mut timeline = Vec::new();

    let conn_summary = conn_summary_handle.summary().unwrap();
    let http_conn_time = conn_summary.connected_time().unwrap();
    timeline.push(CheckTimeSpan {
        name: "tls-handshake",
        started_at: conn_summary.start_time(),
        ended_at: http_conn_time,
    });
    timeline.push(CheckTimeSpan {
        name: "http",
        started_at: http_conn_time,
        ended_at: finish_time,
    });

    let status = resp.status();

    let mut is_failed = status.is_server_error();
    if !check.allow_client_error {
        is_failed |= status.is_client_error();
    }

    if is_failed {
        return CheckResult::new(
            CHECK_NAME,
            CheckResultElement {
                target: ip_addr,
                check_status: CheckStatus::Failed,
                status_code: status.as_u16(),
                err_msg: Some(format!("bad HTTP status {}", status)),
                timeline,
            },
        );
    }

    let mut check_status = CheckStatus::Alive;

    let mut err_msg = None;

    let total_latency = finish_time - conn_summary.start_time();
    if check.latency_requirement < total_latency {
        check_status = CheckStatus::Degraded;
        err_msg = Some(format!("High latency: {:?}", total_latency));
    }

    CheckResult::new(
        CHECK_NAME,
        CheckResultElement {
            target: ip_addr,
            check_status,
            status_code: status.as_u16(),
            err_msg,
            timeline,
        },
    )
}
