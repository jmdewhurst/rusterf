#![warn(clippy::pedantic)]

use zmq;

#[derive(Debug)]
pub struct Comms {
    ctx: zmq::Context,
    logs_sock: zmq::Socket,
}
