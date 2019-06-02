use ws::{Factory, Handler, Sender};
use crate::CanaryConfig;

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
