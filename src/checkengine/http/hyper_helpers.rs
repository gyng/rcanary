use std::io;
use std::net::IpAddr;
use std::sync::Arc;
use std::sync::Mutex;
use std::time::Instant;

use futures::compat::Compat;
use futures::future::{self, Ready};
use futures01::future::Future as LegacyFuture;
use futures01::{Async, Poll};
use hyper::client::connect::dns::{Name, Resolve};
use hyper::client::connect::{Connect, Connected, Destination};
use hyper_tls::{HttpsConnector, MaybeHttpsStream};

#[derive(Clone)]
pub(super) struct StaticResolverSingle {
    ip_addr: IpAddr,
}

impl StaticResolverSingle {
    pub fn new(ip_addr: IpAddr) -> StaticResolverSingle {
        StaticResolverSingle { ip_addr }
    }
}

impl Resolve for StaticResolverSingle {
    type Addrs = std::vec::IntoIter<IpAddr>;
    type Future = Compat<Ready<Result<Self::Addrs, io::Error>>>;

    fn resolve(&self, _name: Name) -> Self::Future {
        Compat::new(future::ok(vec![self.ip_addr].into_iter()))
    }
}

#[derive(Clone)]
pub struct ConnectSummaryHandle {
    inner: Arc<Mutex<Option<ConnectSummary>>>,
}

#[derive(Clone, Debug)]
pub struct ConnectSummary {
    start_time: Instant,
    connected_time: Option<Instant>,
}

impl ConnectSummary {
    pub fn start_time(&self) -> Instant {
        self.start_time
    }

    pub fn connected_time(&self) -> Option<Instant> {
        self.connected_time
    }
}

impl ConnectSummaryHandle {
    pub fn summary(&self) -> Option<ConnectSummary> {
        let s = self.inner.lock().unwrap();
        s.as_ref().cloned()
    }
}

/// HttpsConnectorWrapped lets us measure TLS handshake time.
pub(super) struct HttpsConnectorWrapped<T> {
    inner: HttpsConnector<T>,
    conn_summary: ConnectSummaryHandle,
}

impl<T> HttpsConnectorWrapped<T> {
    pub fn summary_handle(&self) -> ConnectSummaryHandle {
        self.conn_summary.clone()
    }
}

impl<T> From<HttpsConnector<T>> for HttpsConnectorWrapped<T> {
    fn from(inner: HttpsConnector<T>) -> HttpsConnectorWrapped<T> {
        HttpsConnectorWrapped {
            inner,
            conn_summary: ConnectSummaryHandle {
                inner: Arc::new(Mutex::new(None)),
            },
        }
    }
}

impl<T> Connect for HttpsConnectorWrapped<T>
where
    T: Connect<Error = io::Error>,
    T::Transport: 'static,
    T::Future: 'static,
{
    type Transport = MaybeHttpsStream<T::Transport>;
    type Error = io::Error;
    type Future = HttpsConnectingWrapped<T::Transport>;

    fn connect(&self, dest: Destination) -> Self::Future {
        let inner_fut = self.inner.connect(dest);
        {
            let mut cs = self.conn_summary.inner.lock().unwrap();
            *cs = Some(ConnectSummary {
                start_time: Instant::now(),
                connected_time: None,
            });
        }
        HttpsConnectingWrapped {
            inner_fut,
            conn_summary: self.conn_summary.clone(),
        }
    }
}

pub(super) struct HttpsConnectingWrapped<T> {
    inner_fut: hyper_tls::HttpsConnecting<T>,
    conn_summary: ConnectSummaryHandle,
}

impl<T> LegacyFuture for HttpsConnectingWrapped<T> {
    type Item = (hyper_tls::MaybeHttpsStream<T>, Connected);
    type Error = io::Error;

    fn poll(&mut self) -> Poll<Self::Item, Self::Error> {
        match self.inner_fut.poll() {
            Ok(Async::Ready(v)) => {
                let mut cs_handle = self.conn_summary.inner.lock().unwrap();
                let cs = cs_handle.as_mut().unwrap();
                cs.connected_time = Some(Instant::now());
                Ok(Async::Ready(v))
            }
            Ok(Async::NotReady) => Ok(Async::NotReady),
            Err(err) => Err(err),
        }
    }
}
