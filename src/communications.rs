#![warn(clippy::pedantic)]
// use std::io::Read;

use chrono::Local;
use futures::future::FutureExt;
use gethostname::gethostname;
// use serde::{Deserialize, Serialize};
use bytes::Bytes;
use zeromq::prelude::*;

use super::interferometer::Interferometer;

use std::str;

fn iterf32_to_bytes<C>(collection: C) -> Bytes
where
    C: IntoIterator<Item = f32>,
{
    collection
        .into_iter()
        .flat_map(f32::to_le_bytes)
        .collect::<Vec<u8>>()
        .into()
}
fn vecu32_to_bytes(collection: &[u32]) -> Bytes {
    collection
        .iter()
        .flat_map(|x| x.to_le_bytes())
        .collect::<Vec<u8>>()
        .into()
}

pub struct InterfComms {
    hostname: String,
    logs_sock: zeromq::PubSocket,
    logs_port: u16,
    command_sock: zeromq::RepSocket,
    command_port: u16,
    logs_publish_frequency_exponent: u8,
}

// fn vf32_to_u8(v: &[f32]) -> &[u8] {
//     unsafe { std::slice::from_raw_parts(v.as_ptr().cast::<u8>(), v.len() * 4) }
// }
// fn vu32_to_u8(v: &[u32]) -> &[u8] {
//     unsafe { std::slice::from_raw_parts(v.as_ptr().cast::<u8>(), v.len() * 4) }
// }
fn floor_exp(num: u32) -> u8 {
    let mut exp = 0;
    while 1 << exp < num {
        exp += 1;
    }
    if 1 << exp > num {
        exp -= 1;
    }
    exp
}

impl InterfComms {
    #[must_use]
    pub fn new() -> Option<Self> {
        let logs_sock = zeromq::PubSocket::new();
        let command_sock = zeromq::RepSocket::new();
        // let msg_outgoing = zmq::Message::new();
        let hostname = gethostname().into_string().ok()?;
        Some(InterfComms {
            hostname,
            logs_sock,
            logs_port: 8080,
            command_sock,
            command_port: 8081,
            logs_publish_frequency_exponent: 8,
        })
    }

    pub fn set_log_publish_frequency(&mut self, num_cycles: u32) {
        // round `num_cycles` down to the nearest power of 2
        self.logs_publish_frequency_exponent = floor_exp(num_cycles);
    }

    #[inline]
    #[must_use]
    pub fn should_publish_logs(&self, num_cycles: u64) -> bool {
        (num_cycles >> self.logs_publish_frequency_exponent) == 0
    }

    pub async fn handle_socket_request(&mut self, interf: &mut Interferometer) -> Option<String> {
        let cmd_msg = self.command_sock.recv().now_or_never()?.ok()?;
        let cmd = str::from_utf8(cmd_msg.get(0)?).ok()?;
        let _ = if let Ok(s) = interf.process_command(cmd.split(':')) {
            self.command_sock.send(s.into()).await
        } else {
            eprintln!("[{}] failed to process command [{}]", Local::now(), cmd);
            self.command_sock.send("".into()).await
        };
        Some(cmd.to_string())
    }

    /// Poll the command socket, and handle a command if one is queued up. Returns Some()
    /// containing the text of the message if one was found, or None if polling failed or there
    /// was no message to be processed, or we failed to handle the command.
    //pub fn handle_socket_request(&mut self, interf: &mut Interferometer) -> Option<&str> {
    //    //TODO: switch return type to Result<&str,zmq::Error> ?
    //    match self.command_sock.poll(zmq::POLLIN, 0) {
    //        Err(_x) => None,
    //        Ok(0) => None,
    //        Ok(_x) => {
    //            self.command_sock.recv(&mut self.msg_incoming, 0).ok()?;
    //            match interf.process_command(
    //                self.msg_incoming
    //                    .as_str()
    //                    .expect("already checked .ok()")
    //                    .split(':'),
    //            ) {
    //                Ok(None) => self.command_sock.send("", 0).ok()?,
    //                Ok(Some(s)) => self.command_sock.send(&s, 0).ok()?,
    //                Err(_) => {
    //                    eprintln!(
    //                        "[{}] failed to process command [{}]",
    //                        Local::now(),
    //                        self.msg_incoming.as_str().expect("already checked .ok()")
    //                    );
    //                    self.command_sock.send("", 0).ok()?;
    //                }
    //            };
    //            return Some(self.msg_incoming.as_str().expect("already checked .ok()"));
    //        }
    //    }
    //}

    /// # Errors
    /// Propagates any zeromq error in the socket send operation.
    pub async fn publish_logs(&mut self, interf: &mut Interferometer) -> zeromq::ZmqResult<()> {
        let mut msg: zeromq::ZmqMessage = self.hostname.clone().into();

        msg.push_back(interf.cycle_counter.to_le_bytes().to_vec().into());

        msg.push_back(iterf32_to_bytes(&interf.ref_laser.phase_log));
        msg.push_back(iterf32_to_bytes(&interf.slave_laser.phase_log));
        msg.push_back(iterf32_to_bytes(&interf.ref_laser.feedback_log));
        msg.push_back(iterf32_to_bytes(&interf.slave_laser.feedback_log));

        msg.push_back(vecu32_to_bytes(&interf.last_waveform_ref));
        msg.push_back(vecu32_to_bytes(&interf.last_waveform_slave));

        msg.push_back(iterf32_to_bytes(interf.ref_laser.fit_coefficients));
        msg.push_back(iterf32_to_bytes(interf.slave_laser.fit_coefficients));

        self.logs_sock.send(msg).await
    }

    /// # Errors
    /// In case of any zmq error, aborts early and returns the error.
    // pub fn publish_logs(&mut self, interf: &mut Interferometer) -> Result<(), zeromq::Error> {
    //     self.logs_sock.send(&self.hostname, zmq::SNDMORE)?;
    //     self.logs_sock
    //         .send(interf.cycle_counter.to_le_bytes().as_slice(), zmq::SNDMORE)?;

    //     // syntax is a mess, but I think doing it this way avoids unnecessary allocations.
    //     // want to ensure that ``outgoing_values`` has enough space to hold the phase logs
    //     self.outgoing_buffer.clear();
    //     self.outgoing_buffer
    //         .reserve_exact(interf.ref_laser.phase_log.len());

    //     // consider switching these to manual overwriting to avoid unnecessary clears
    //     self.outgoing_buffer.extend(&interf.ref_laser.phase_log);
    //     self.logs_sock
    //         .send(vf32_to_u8(&self.outgoing_buffer), zmq::SNDMORE)?;
    //     self.outgoing_buffer.clear();
    //     self.outgoing_buffer.extend(&interf.slave_laser.phase_log);
    //     self.logs_sock
    //         .send(vf32_to_u8(&self.outgoing_buffer), zmq::SNDMORE)?;
    //     self.outgoing_buffer.clear();
    //     self.outgoing_buffer.extend(&interf.ref_laser.feedback_log);
    //     self.logs_sock
    //         .send(vf32_to_u8(&self.outgoing_buffer), zmq::SNDMORE)?;
    //     self.outgoing_buffer.clear();
    //     self.outgoing_buffer
    //         .extend(&interf.slave_laser.feedback_log);
    //     self.logs_sock
    //         .send(vf32_to_u8(&self.outgoing_buffer), zmq::SNDMORE)?;

    //     self.logs_sock
    //         .send(vu32_to_u8(&interf.last_waveform_ref), zmq::SNDMORE)?;
    //     self.logs_sock
    //         .send(vu32_to_u8(&interf.last_waveform_slave), zmq::SNDMORE)?;

    //     self.logs_sock
    //         .send(vf32_to_u8(&interf.ref_laser.fit_coefficients), zmq::SNDMORE)?;
    //     self.logs_sock
    //         .send(vf32_to_u8(&interf.slave_laser.fit_coefficients), 0)?;

    //     Ok(())
    // }

    /// # Errors
    /// In case of any zmq error, aborts early and returns the error.
    pub async fn bind_sockets(
        &mut self,
        log_port: u16,
        command_port: u16,
    ) -> zeromq::ZmqResult<()> {
        self.logs_sock
            .bind(format!("tcp://0.0.0.0:{log_port}").as_str())
            .await?;
        self.logs_port = log_port;
        self.command_sock
            .bind(format!("tcp://0.0.0.0:{command_port}").as_str())
            .await?;
        self.command_port = command_port;
        Ok(())
    }

    /// # Errors
    /// In case of any zmq error, aborts early and returns the error.
    pub async fn unbind_sockets(&mut self) -> zeromq::ZmqResult<()> {
        let _ = self.logs_sock.unbind_all().await;
        let _ = self.command_sock.unbind_all().await;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn floor_exp_test() {
        assert_eq!(floor_exp(1), 0);
        assert_eq!(floor_exp(2), 1);
        assert_eq!(floor_exp(3), 1);
        assert_eq!(floor_exp(4), 2);
        assert_eq!(floor_exp(2048), 11);
    }
}

// #[derive(Serialize, Deserialize, Debug)]
// struct CommsSerialize {
//     hostname: String,
//     logs_port: u16,
//     command_port: u16,
// }

// impl CommsSerialize {
//     fn into_interf_comms(self) -> Option<InterfComms> {
//         if gethostname().into_string().ok()? == self.hostname {
//             let ctx = zmq::Context::new();
//             let mut comms = InterfComms::new(&ctx)?;
//             comms.bind_sockets(self.logs_port, self.command_port).ok()?;
//             return Some(comms);
//         }
//         None
//     }

//     fn from_interf_comms(comms: &InterfComms) -> Self {
//         CommsSerialize {
//             hostname: comms.hostname.to_string(),
//             logs_port: comms.logs_port,
//             command_port: comms.command_port,
//         }
//     }
// }

// impl<'de> Deserialize<'de> for InterfComms {
//     fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
//         Ok(CommsSerialize::deserialize(d)?.into_interf_comms().unwrap())
//     }
// }

// impl Serialize for InterfComms {
//     fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
//     where
//         S: serde::Serializer,
//     {
//         CommsSerialize::from_interf_comms(self).serialize(serializer)
//     }
// }
