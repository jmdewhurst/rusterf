#![warn(clippy::pedantic)]
use serde::{Deserialize, Serialize};

use gethostname::gethostname;

use super::interferometer::Interferometer;
use zmq;

pub struct InterfComms {
    hostname: String,
    ctx: zmq::Context,
    logs_sock: zmq::Socket,
    logs_port: u16,
    command_sock: zmq::Socket,
    command_port: u16,
    msg_incoming: zmq::Message,
    outgoing_buffer: Vec<f32>,
}

fn vf_to_u8(v: &[f32]) -> &[u8] {
    unsafe { std::slice::from_raw_parts(v.as_ptr().cast::<u8>(), v.len() * 4) }
}
fn vu32_to_u8(v: &[u32]) -> &[u8] {
    unsafe { std::slice::from_raw_parts(v.as_ptr().cast::<u8>(), v.len() * 4) }
}

impl InterfComms {
    // TODO: eliminate panics?
    pub fn new(ctx: zmq::Context) -> Self {
        let logs_sock = ctx.socket(zmq::PUB).unwrap();
        let command_sock = ctx.socket(zmq::REP).unwrap();
        let msg_incoming = zmq::Message::new();
        // let msg_outgoing = zmq::Message::new();
        let hostname = gethostname().into_string().unwrap();
        InterfComms {
            hostname,
            ctx,
            logs_sock,
            logs_port: 8080,
            command_sock,
            command_port: 8081,
            msg_incoming,
            outgoing_buffer: Vec::new(),
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
                if self.command_sock.recv(&mut self.msg_incoming, 0).is_ok() {
                    match interf.process_command(self.msg_incoming.as_str().unwrap().split(':')) {
                        Ok(None) => self.command_sock.send("", 0).ok()?,
                        Ok(Some(s)) => self.command_sock.send(&s, 0).ok()?,
                        Err(_y) => self.command_sock.send("", 0).ok()?,
                    };
                    return Some(self.msg_incoming.as_str().unwrap());
                }
                None
            }
        }
    }

    pub fn publish_logs(&mut self, interf: &mut Interferometer) -> Result<(), zmq::Error> {
        self.logs_sock.send(&self.hostname, zmq::SNDMORE)?;
        self.logs_sock
            .send(interf.cycle_counter.to_le_bytes().as_slice(), zmq::SNDMORE)?;

        // syntax is a mess, but I think doing it this way avoids unnecessary allocations.
        // want to ensure that ``outgoing_values`` has enough space to hold the phase logs
        self.outgoing_buffer.clear();
        self.outgoing_buffer
            .reserve_exact(interf.ref_laser.phase_log.len());

        // consider switching these to manual overwriting to avoid unnecessary clears
        self.outgoing_buffer.extend(&interf.ref_laser.phase_log);
        self.logs_sock
            .send(vf_to_u8(&self.outgoing_buffer), zmq::SNDMORE)?;
        self.outgoing_buffer.clear();
        self.outgoing_buffer.extend(&interf.slave_laser.phase_log);
        self.logs_sock
            .send(vf_to_u8(&self.outgoing_buffer), zmq::SNDMORE)?;
        self.outgoing_buffer.clear();
        self.outgoing_buffer.extend(&interf.ref_laser.feedback_log);
        self.logs_sock
            .send(vf_to_u8(&self.outgoing_buffer), zmq::SNDMORE)?;
        self.outgoing_buffer.clear();
        self.outgoing_buffer
            .extend(&interf.slave_laser.feedback_log);
        self.logs_sock
            .send(vf_to_u8(&self.outgoing_buffer), zmq::SNDMORE)?;

        self.logs_sock
            .send(vu32_to_u8(&interf.last_waveform_chA), zmq::SNDMORE)?;
        self.logs_sock
            .send(vu32_to_u8(&interf.last_waveform_chB), zmq::SNDMORE)?;

        self.logs_sock
            .send(vf_to_u8(&interf.ref_laser.fit_coefficients), zmq::SNDMORE)?;
        self.logs_sock
            .send(vf_to_u8(&interf.slave_laser.fit_coefficients), 0)?;

        Ok(())
    }

    pub fn bind_sockets(&mut self, log_port: u16, command_port: u16) -> zmq::Result<()> {
        self.logs_sock
            .bind(format!("tcp://*.{}", log_port).as_str())?;
        self.logs_port = log_port;
        self.command_sock
            .bind(format!("tcp://*.{}", command_port).as_str())?;
        self.command_port = command_port;
        Ok(())
    }

    pub fn unbind_sockets(&mut self) -> zmq::Result<()> {
        self.logs_sock
            .unbind(format!("tcp://*.{}", self.logs_port).as_str())?;
        self.command_sock
            .unbind(format!("tcp://*.{}", self.command_port).as_str())?;
        Ok(())
    }
}

#[derive(Serialize, Deserialize, Debug)]
struct CommsSerialize {
    hostname: String,
    logs_port: u16,
    command_port: u16,
}

impl CommsSerialize {
    fn into_interf_comms(self) -> Option<InterfComms> {
        if gethostname().into_string().ok()? == self.hostname {
            let ctx = zmq::Context::new();
            let mut comms = InterfComms::new(ctx);
            comms.bind_sockets(self.logs_port, self.command_port).ok()?;
            return Some(comms);
        }
        None
    }

    fn from_interf_comms(comms: &InterfComms) -> Self {
        CommsSerialize {
            hostname: comms.hostname.to_string(),
            logs_port: comms.logs_port,
            command_port: comms.command_port,
        }
    }
}

impl<'de> Deserialize<'de> for InterfComms {
    // TODO: handle error case properly, rather than just unwrapping
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        Ok(CommsSerialize::deserialize(d)?.into_interf_comms().unwrap())
    }
}

impl Serialize for InterfComms {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        CommsSerialize::from_interf_comms(self).serialize(serializer)
    }
}
