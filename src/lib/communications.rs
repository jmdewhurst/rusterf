#![warn(clippy::pedantic)]

use super::interferometer::Interferometer;
use zmq;

pub struct InterfComms {
    ctx: zmq::Context,
    logs_sock: zmq::Socket,
    logs_port: String,
    command_sock: zmq::Socket,
    command_port: String,
    msg: zmq::Message,
}

impl InterfComms {
    pub fn new(ctx: zmq::Context) -> Self {
        let logs_sock = ctx.socket(zmq::PUB).unwrap();
        let command_sock = ctx.socket(zmq::REP).unwrap();
        let msg = zmq::Message::new();
        InterfComms {
            ctx,
            logs_sock,
            logs_port: "8080".to_owned(),
            command_sock,
            command_port: "8081".to_owned(),
            msg,
        }
    }

    /// Poll the command socket, and handle a command if one is queued up. Returns Some()
    /// containing the text of the message if one was found, or None if polling failed or there
    /// was no message to be processed, or we failed to handle the command.
    pub fn handle_socket_request(&mut self, interf: &mut Interferometer) -> Option<&str> {
        match self.command_sock.poll(zmq::POLLIN, 0) {
            Err(_x) => None,
            Ok(0) => None,
            Ok(_x) => {
                if self.command_sock.recv(&mut self.msg, 0).is_ok() {
                    interf
                        .process_command(self.msg.as_str().unwrap().split(':'))
                        .ok()?;
                    return Some(self.msg.as_str().unwrap());
                }
                None
            }
        }
    }

    pub fn bind_sockets(&mut self, log_port: &str, command_port: &str) -> zmq::Result<()> {
        self.logs_sock
            .bind(format!("tcp://*.{}", log_port).as_str())?;
        self.logs_port = log_port.to_owned();
        self.command_sock
            .bind(format!("tcp://*.{}", command_port).as_str())?;
        self.command_port = command_port.to_owned();
        Ok(())
    }

    pub fn unbind_sockets(&mut self) -> zmq::Result<()> {
        self.logs_sock.unbind(self.logs_port.as_str())?;
        self.command_sock.unbind(self.command_port.as_str())?;
        Ok(())
    }
}
