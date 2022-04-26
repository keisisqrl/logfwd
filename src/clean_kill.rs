//! # clean_kill
//! 
//! Provides a signal handler on SIGTERM that shuts down cleanly

use crate::{Error, FwdMsg, Shutdown};
use libsystemd::daemon;
use log::info;
use tokio::{signal::unix::{signal, Signal, SignalKind}, sync::{mpsc::Sender, broadcast}};

use std::{
    future::Future,
    pin::Pin,
    task::{Context, Poll},
};

/// A future/task that returns only after the program receives a SIGTERM
/// and messages have been sent to all other loops.
pub struct Handler {
    signal: Signal,
    channel: Sender<FwdMsg>,
    bcast_send: broadcast::Sender<Shutdown>
}

impl Handler {
    /// Create a new Handler
    /// Takes a reference to the send half of the `mpsc` channel
    /// and one to the `broadcast` channel.
    pub fn new(parent_send: &Sender<FwdMsg>, bcast_send: &broadcast::Sender<Shutdown>) -> Self {
        Self {
            signal: signal(SignalKind::terminate()).unwrap(),
            channel: parent_send.clone(),
            bcast_send: bcast_send.clone()
        }
    }
}

impl Future for Handler {
    type Output = Result<(),Error>;

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        if let Poll::Pending = self.signal.poll_recv(cx) {
            Poll::Pending
        } else {
            info!(target: "clean_kill", "sigterm received, shutting down");
            daemon::notify(false, &[daemon::NotifyState::Stopping]).unwrap();
            self.bcast_send.send(Shutdown).unwrap();
            self.channel.try_send(FwdMsg::Close).unwrap();
            Poll::Ready(Ok(()))
        }
    }
}
