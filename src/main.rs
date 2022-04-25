use std::env;
use std::os::unix::io::{IntoRawFd};

use libsystemd::daemon::NotifyState;
use libsystemd::{activation, daemon};

use tokio::sync::mpsc;

use logfwd::udp_recv::Receiver;
use logfwd::tls_send::Sender;
use logfwd::clean_kill;

use log::{LevelFilter, debug, trace};

#[tokio::main]
async fn main() {
    systemd_journal_logger::init().unwrap();

    log::set_max_level(LevelFilter::Info);

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

    let udp_side = Receiver::new(raw_udp_fd,&chan_send);
    
    trace!(target: "main", "Set up UDP receiver");

    let mut tls_side = Sender::new(dest_host,dest_port,chan_recv);

    trace!(target: "main", "Set up TLS sender");

    let flusher = logfwd::drano::Flusher::new(5000,&chan_send);

    trace!(target: "main", "Set up interval flusher");

    debug!(target: "main", "Inited structs");

    tls_side.init().await.unwrap();

    debug!(target: "main", "Started TLS connection");

    let _udp_task = tokio::spawn(
        async move {udp_side.run().await}
    );

    trace!(target: "main", "spawned UDP loop");

    let tls_task = tokio::spawn(
        async move {tls_side.run().await}
    );

    trace!(target: "main", "spawned TLS loop");

    let _flush_task = tokio::spawn(
        flusher
    );

    trace!(target: "main", "spawned flusher loop");

    let interceptor = clean_kill::Handler::new(&chan_send);

    let _sig_intercept = tokio::spawn(
        interceptor
    );

    trace!(target:"main", "spawned signal listener");

    debug!(target: "main", "all spawned, notifying systemd");

    daemon::notify(false,&[NotifyState::Ready]).unwrap();

    debug!(target: "main", "systemd notified, joining all tasks");

    //sig_intercept.await.unwrap();
    tls_task.await.unwrap().unwrap();
    return;
    
}