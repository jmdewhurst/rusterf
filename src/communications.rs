#![warn(clippy::pedantic)]
// use std::io::Read;

use chrono::Local;
use gethostname::gethostname;

use super::configs::floor_exp;
use super::interferometer::Interferometer;

fn iterf32_to_bytes<C>(collection: C) -> Vec<u8>
where
    C: IntoIterator<Item = f32>,
{
    collection.into_iter().flat_map(f32::to_le_bytes).collect()
}
fn slu32_to_bytes(collection: &[u32]) -> Vec<u8> {
    collection.iter().flat_map(|x| x.to_le_bytes()).collect()
}

pub struct InterfComms {
    pub ctx: zmq::Context,
    hostname: String,
    logs_sock: zmq::Socket,
    logs_port: u16,
    command_sock: zmq::Socket,
    command_port: u16,
    msg_incoming: zmq::Message,
    logs_publish_frequency_exponent: u8,
}

impl InterfComms {
    #[must_use]
    pub fn new() -> Option<Self> {
        let ctx = zmq::Context::new();
        let logs_sock = ctx.socket(zmq::PUB).ok()?;
        let command_sock = ctx.socket(zmq::REP).ok()?;
        let msg_incoming = zmq::Message::new();
        let hostname = gethostname().into_string().ok()?;
        Some(InterfComms {
            ctx,
            hostname,
            logs_sock,
            logs_port: 8080,
            command_sock,
            command_port: 8081,
            msg_incoming,
            logs_publish_frequency_exponent: 8,
        })
    }

    pub fn set_log_publish_frequency(&mut self, num_cycles: u32) {
        // round `num_cycles` down to the nearest power of 2
        self.logs_publish_frequency_exponent = floor_exp(u64::from(num_cycles));
    }

    #[inline]
    #[must_use]
    pub fn should_publish_logs(&self, num_cycles: u64) -> bool {
        (num_cycles & ((1 << self.logs_publish_frequency_exponent) - 1)) == 0
    }

    /// Poll the command socket, and handle a command if one is queued up. Returns Some()
    /// containing the text of the message if one was found, or None if polling failed or there
    /// was no message to be processed, or we failed to handle the command.
    pub fn handle_socket_request(&mut self, interf: &mut Interferometer) -> Option<String> {
        self.command_sock.poll(zmq::POLLIN, 0).ok()?;
        self.command_sock.recv(&mut self.msg_incoming, 0).ok()?;
        if let Some(msg_str) = self.msg_incoming.as_str() {
            if let Ok(s) = interf.process_command(msg_str.split(':')) {
                self.command_sock.send(&s, 0).ok()?;
                Some(s)
            } else {
                eprintln!("[{}] failed to process command [{msg_str}]", Local::now());
                self.command_sock.send(Vec::new(), 0).ok()?;
                Some(msg_str.into())
            }
        } else {
            eprintln!("[{}] received garbled command", Local::now());
            None
        }
    }

    /// # Errors
    /// In case of any zmq error, aborts early and returns the error.
    pub fn publish_logs(&mut self, interf: &mut Interferometer) -> Result<(), zmq::Error> {
        self.logs_sock.send(&self.hostname, zmq::SNDMORE)?;
        self.logs_sock
            .send(interf.cycle_counter.to_le_bytes().as_slice(), zmq::SNDMORE)?;

        self.logs_sock
            .send(iterf32_to_bytes(&interf.ref_laser.phase_log), zmq::SNDMORE)?;
        self.logs_sock.send(
            iterf32_to_bytes(&interf.slave_laser.phase_log),
            zmq::SNDMORE,
        )?;
        self.logs_sock.send(
            iterf32_to_bytes(&interf.ref_laser.feedback_log),
            zmq::SNDMORE,
        )?;
        self.logs_sock.send(
            iterf32_to_bytes(&interf.slave_laser.feedback_log),
            zmq::SNDMORE,
        )?;

        self.logs_sock
            .send(slu32_to_bytes(&interf.last_waveform_ref), zmq::SNDMORE)?;
        self.logs_sock
            .send(slu32_to_bytes(&interf.last_waveform_slave), zmq::SNDMORE)?;

        self.logs_sock.send(
            iterf32_to_bytes(interf.ref_laser.fit_coefficients),
            zmq::SNDMORE,
        )?;
        self.logs_sock
            .send(iterf32_to_bytes(interf.slave_laser.fit_coefficients), 0)?;

        Ok(())
    }

    /// # Errors
    /// In case of any zmq error, aborts early and returns the error.
    pub fn bind_sockets(&mut self, log_port: u16, command_port: u16) -> Result<(), zmq::Error> {
        self.logs_sock.bind(&format!("tcp://*:{log_port}"))?;
        self.logs_port = log_port;
        self.command_sock.bind(&format!("tcp://*:{command_port}"))?;
        self.command_port = command_port;
        Ok(())
    }

    /// # Errors
    /// In case of any zmq error, aborts early and returns the error.
    pub fn unbind_sockets(&mut self) -> Result<(), zmq::Error> {
        let _ = self
            .logs_sock
            .unbind(&format!("tcp://*:{}", self.logs_port));
        let _ = self
            .command_sock
            .unbind(&format!("tcp://*:{}", self.command_port));
        Ok(())
    }
}
