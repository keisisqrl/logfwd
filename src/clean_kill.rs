use crate::FwdMsg;
use libsystemd::daemon;
use log::info;
use tokio::signal::unix::{signal, Signal, SignalKind};
use tokio::sync::mpsc::Sender;

use std::{
    future::Future,
    pin::Pin,
    task::{Context, Poll},
};

pub struct Handler {
    signal: Signal,
    channel: Sender<FwdMsg>,
}

impl Handler {
    pub fn new(parent_send: &Sender<FwdMsg>) -> Self {
        Self {
            signal: signal(SignalKind::interrupt()).unwrap(),
            channel: parent_send.clone(),
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
