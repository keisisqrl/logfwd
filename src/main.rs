use std::env;
use std::os::unix::io::{IntoRawFd};

use libsystemd::daemon::NotifyState;
use libsystemd::{activation, daemon};

use tokio::sync::mpsc;

use logfwd::udp_recv::Receiver;
use logfwd::tls_send::Sender;
use logfwd::clean_kill::kill_handler;

use log::{info, LevelFilter, debug, trace};

#[tokio::main]
async fn main() {
    systemd_journal_logger::init().unwrap();

    log::set_max_level(LevelFilter::Trace);

    debug!("Init journald logging");

    let dest_port: u16 = env::var("DEST_PORT")
        .unwrap().parse().unwrap();
    let dest_host = env::var("DEST_HOST").unwrap();

    trace!("Got destination: {}:{}", dest_host, dest_port);

    let sockets = activation::receive_descriptors(false).unwrap();
    assert_eq!(sockets.len(), 1);
    let raw_udp_fd = sockets[0].clone().into_raw_fd();

    trace!("Got Fd: {}", raw_udp_fd);
    debug!(target: "main", "Init from systemd done");

    let (chan_send, chan_recv) = mpsc::channel(1024);

    trace!(target: "main", "Set up mpsc");

    let sender_clone = chan_send.clone();

    let udp_side = Receiver::new(raw_udp_fd,sender_clone);
    
    trace!(target: "main", "Set up UDP receiver");

    let mut tls_side = Sender::new(dest_host,dest_port,chan_recv);

    trace!(target: "main", "Set up TLS sender");

    let sender_clone=chan_send.clone();

    let flusher = logfwd::drano::Flusher::new(5000,sender_clone);

    trace!(target: "main", "Set up interval flusher");

    debug!(target: "main", "Inited structs");

    tls_side.init().await.unwrap();

    debug!(target: "main", "Started TLS connection");

    let udp_task = tokio::spawn(
        async move {udp_side.run().await}
    );

    trace!(target: "main", "spawned UDP loop");

    let tls_task = tokio::spawn(
        async move {tls_side.run().await}
    );

    trace!(target: "main", "spawned TLS loop");

    let flush_task = tokio::spawn(
        async move {flusher.run().await}
    );

    trace!(target: "main", "spawned flusher loop");

    let sig_intercept = tokio::spawn(
        async move{kill_handler(tls_task,chan_send).await}
    );

    trace!(target:"main", "spawned signal listener");

    debug!(target: "main", "all spawned, notifying systemd");

    daemon::notify(false,&[NotifyState::Ready]).unwrap();

    debug!(target: "main", "systemd notified, joining all tasks");

    sig_intercept.await.unwrap().unwrap();
}