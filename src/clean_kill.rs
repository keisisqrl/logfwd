use crate::FwdMsg;
use libsystemd::daemon;
use tokio::signal::unix::{ signal, Signal, SignalKind};
use tokio::sync::mpsc::Sender;

use log::{debug, info, trace};
use std::{
    error::Error,
    fmt,
    future::Future,
    pin::Pin,
    task::{Context, Poll},
};

#[derive(Debug)]
struct Exiting;

impl fmt::Display for Exiting {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Exiting due to signal")
    }
}

impl Error for Exiting {}

pub struct Handler {
    signal: Signal,
    channel: Sender<FwdMsg>,
}

impl Handler {
    pub fn new(parent_send: &Sender<FwdMsg>) -> Self {
        Self {
            signal: signal(SignalKind::interrupt()).unwrap(),
            channel: parent_send.clone()
        }
    }
}

impl Future for Handler {
    type Output = ();

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        if let Poll::Pending = self.signal.poll_recv(cx) {
            Poll::Pending
        } else {
            info!(target: "clean_kill", "sigterm received, shutting down");
            daemon::notify(false, &[daemon::NotifyState::Stopping]).unwrap();
            self.channel.try_send(FwdMsg::Close).unwrap();
            Poll::Ready(())
        }
    }
}
