// use std::net::SocketAddr;
// use futures::future::Future;

// use super::{Check, CheckFailure};

// pub struct TcpConnectCheck;

// impl Check for TcpConnectCheck {
//     type Target = SocketAddr;
//     type Future = Box<dyn Future<Output=Result<(), CheckFailure>> + Unpin>;

//     fn check(&self, target: Self::Target) -> Self::Future {
//         unimplemented!();
//     }
// }
