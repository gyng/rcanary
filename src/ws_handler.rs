extern crate ws;

use ws::{Factory, Handler, Sender};

pub struct ClientHandler;

impl Handler for ClientHandler {}

pub struct ClientFactory;

impl Factory for ClientFactory {
    type Handler = ClientHandler;

    fn connection_made(&mut self, _ws: Sender) -> ClientHandler {
        ClientHandler {}
    }

    fn client_connected(&mut self, _ws: Sender) -> ClientHandler {
        ClientHandler {}
    }
}
