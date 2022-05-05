use logfwd::{clean_kill, tls_send::Sender, udp_recv::Receiver};

use std::os::unix::prelude::RawFd;
use std::{env, os::unix::io::IntoRawFd};
use tokio::sync::{broadcast, mpsc};
use tokio::try_join;
use tracing::{debug, trace};

#[cfg(target_os = "linux")]
use libsystemd::{
    activation,
    daemon::{self, NotifyState},
};

#[cfg(target_os = "macos")]
use raunch;

#[tokio::main]
async fn main() {
    console_subscriber::init();

    debug!("Init journald logging");

    let dest_port: u16 = env::var("DEST_PORT").unwrap().parse().unwrap();
    let dest_host = env::var("DEST_HOST").unwrap();

    trace!("Got destination: {}:{}", dest_host, dest_port);

    let sockets: Vec<RawFd> = activate_fds();
    assert_eq!(sockets.len(), 1);
    let raw_udp_fd: RawFd = sockets[0];

    trace!("Got Fd: {}", raw_udp_fd);
    debug!(target: "main", "Init from systemd done");

    let (chan_send, chan_recv) = mpsc::channel(512);

    let (bcast_send, _) = broadcast::channel::<logfwd::Shutdown>(16);

    trace!(target: "main", "Set up mpsc");

    let udp_side = Receiver::new(raw_udp_fd, &chan_send, &bcast_send, 2048);

    trace!(target: "main", "Set up UDP receiver");

    let mut tls_side = Sender::new(dest_host, dest_port, chan_recv);

    trace!(target: "main", "Set up TLS sender");

    let flusher = logfwd::drano::Flusher::new(5000, &chan_send, &bcast_send);

    trace!(target: "main", "Set up interval flusher");

    debug!(target: "main", "Inited structs");

    tls_side.init().await.unwrap();

    debug!(target: "main", "Started TLS connection");

    let udp_task = tokio::spawn(udp_side);

    trace!(target: "main", "spawned UDP loop");

    let tls_task = tokio::spawn(async move { tls_side.run().await });

    trace!(target: "main", "spawned TLS loop");

    let flush_task = tokio::spawn(flusher);

    trace!(target: "main", "spawned flusher loop");

    let interceptor = clean_kill::Handler::new(&chan_send, &bcast_send);

    let sig_intercept = tokio::spawn(interceptor);

    trace!(target:"main", "spawned signal listener");

    debug!(target: "main", "all spawned, notifying systemd");

    #[cfg(target_os = "linux")]
    daemon::notify(false, &[NotifyState::Ready]).unwrap();

    debug!(target: "main", "systemd notified, joining all tasks");

    let (tls_ret, sig_ret, flush_ret, udp_ret) =
        try_join!(tls_task, sig_intercept, flush_task, udp_task).unwrap();

    tls_ret.unwrap();
    sig_ret.unwrap();
    flush_ret.unwrap();
    udp_ret.unwrap();
}

#[cfg(target_os = "linux")]
fn activate_fds() -> Vec<RawFd> {
    activation::receive_descriptors(false)
        .unwrap()
        .iter()
        .map(|x| x.clone().into_raw_fd())
        .collect()
}

#[cfg(target_os = "macos")]
fn activate_fds() -> Vec<RawFd> {
    raunch::activate_socket("logfwd").unwrap()
}
