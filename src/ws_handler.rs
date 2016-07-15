extern crate ws;

use ws::{Factory, Handler, Sender};
use rustc_serialize::json;
use CanaryConfig;

pub struct ClientHandler;

impl Handler for ClientHandler {}

pub struct ClientFactory {
  pub config: CanaryConfig
}

impl Factory for ClientFactory {
    type Handler = ClientHandler;

    fn connection_made(&mut self, ws: Sender) -> ClientHandler {
        let _ = ws.send(json::encode(&self.config.targets).unwrap());
        ClientHandler {}
    }

    fn client_connected(&mut self, _ws: Sender) -> ClientHandler {
        ClientHandler {}
    }
}
