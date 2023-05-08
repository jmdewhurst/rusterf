#![warn(clippy::pedantic)]
// use std::io::Read;

use bytes::Bytes;
use chrono::Local;
use futures::future::FutureExt;
use gethostname::gethostname;
use librp_sys::generator::{Channel, Pulse, DC};
use zeromq::prelude::*;

use super::interferometer::Interferometer;

use std::panic::{catch_unwind, AssertUnwindSafe};
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

    #[inline]
    #[must_use]
    pub fn logs_port(&self) -> u16 {
        self.logs_port
    }
    #[inline]
    #[must_use]
    pub fn command_port(&self) -> u16 {
        self.command_port
    }

    #[inline]
    #[must_use]
    pub fn logs_publish_frequency_exponent(&self) -> u8 {
        self.logs_publish_frequency_exponent
    }

    pub fn set_log_publish_frequency(&mut self, num_cycles: u32) {
        // round `num_cycles` down to the nearest power of 2
        self.logs_publish_frequency_exponent = num_cycles.checked_ilog2().unwrap_or(0) as u8;
    }

    #[inline]
    #[must_use]
    pub fn should_publish_logs(&self, num_cycles: u64) -> bool {
        (num_cycles & ((1 << self.logs_publish_frequency_exponent) - 1)) == 0
    }

    pub async fn handle_socket_request<'a>(
        &mut self,
        interf: &mut Interferometer,
        ramp_ch: Option<&mut Channel<'_, Pulse>>,
        slave_ch: &mut Channel<'_, DC>,
    ) -> Option<String> {
        let cmd_msg = catch_unwind(AssertUnwindSafe(|| self.command_sock.recv().now_or_never()))
            .map_err(|_| async {
                let _ = self.unbind_sockets().await;
                let _ = self.bind_sockets(self.logs_port, self.command_port).await;
            })
            .ok()??
            .ok()?;
        let cmd = str::from_utf8(cmd_msg.get(0)?).ok()?;
        let _ = if let Ok(s) = interf.process_command(cmd.split(':'), ramp_ch, slave_ch) {
            self.command_sock.send(s.into()).await
        } else {
            eprintln!("[{}] failed to process command [{}]", Local::now(), cmd);
            self.command_sock.send(format!("Command '{cmd}' not recognized").into()).await
        };
        Some(cmd.to_string())
    }

    /// # Errors
    /// Propagates any zeromq error in the socket send operation.
    pub async fn publish_logs(
        &mut self,
        interf: &mut Interferometer,
        ref_red_chisq: f32,
        slave_red_chisq: f32,
    ) -> zeromq::ZmqResult<()> {
        let mut msg: zeromq::ZmqMessage = self.hostname.clone().into();

        msg.push_back(interf.cycle_counter.to_le_bytes().to_vec().into());
        msg.push_back(interf.start_time.elapsed().as_secs().to_le_bytes().to_vec().into());

        msg.push_back(iterf32_to_bytes(&interf.ref_laser.phase_log));
        msg.push_back(iterf32_to_bytes(&interf.slave_laser.phase_log));
        msg.push_back(iterf32_to_bytes(&interf.slave_laser.feedback_log));

        msg.push_back(vecu32_to_bytes(&interf.last_waveform_ref));
        msg.push_back(vecu32_to_bytes(&interf.last_waveform_slave));

        msg.push_back(iterf32_to_bytes(interf.ref_laser.fit_coefficients));
        msg.push_back(iterf32_to_bytes(interf.slave_laser.fit_coefficients));

        let stats = interf.stats.evaluate();

        let mut stats_vec = Vec::with_capacity(20);
        stats_vec.extend(stats.avg_fitting_time_us.to_le_bytes());
        stats_vec.extend(stats.avg_iterations_ref.to_le_bytes());
        stats_vec.extend(stats.avg_iterations_slave.to_le_bytes());
        stats_vec.extend(ref_red_chisq.to_le_bytes());
        stats_vec.extend(slave_red_chisq.to_le_bytes());

        msg.push_back(stats_vec.into());

        self.logs_sock.send(msg).await
    }

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
