extern crate ws;
extern crate serde_json;

use ws::{Factory, Handler, Sender};
use CanaryConfig;

pub struct ClientHandler;

impl Handler for ClientHandler {}

pub struct ClientFactory {
    pub config: CanaryConfig,
}

impl Factory for ClientFactory {
    type Handler = ClientHandler;

    fn connection_made(&mut self, ws: Sender) -> ClientHandler {
        let _ = ws.send(serde_json::to_string(&self.config.targets).unwrap());
        ClientHandler {}
    }

    fn client_connected(&mut self, _ws: Sender) -> ClientHandler {
        ClientHandler {}
    }
}
