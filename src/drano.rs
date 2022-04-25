//! # Drano
//! 
//! Contains the future [Flusher].

use crate::FwdMsg;
use futures_util::{Future, never::Never};
use log::trace;
use std::{
    pin::Pin,
    task::{Context, Poll},
    time::Duration,
};
use tokio::{
    sync::mpsc::Sender,
    time::{self, Instant, Sleep},
};
use tokio_util::sync::PollSender;
use pin_project::pin_project;

/// `Flusher` is a custom future which never returns.
/// 
/// Instead, it sends a [FwdMsg::Flush] periodically.
#[pin_project]
pub struct Flusher {
    #[pin]
    sleep: Sleep,
    #[pin]
    sender: PollSender<FwdMsg>,
    period: Duration,
}

impl Flusher {

    /// Create a new [Flusher] future.
    /// 
    /// Takes a `period` in milliseconds and a ref to a [Sender].
    /// 
    /// Run the returned Future with [tokio::spawn]
    pub fn new(period: u64, send_channel: &Sender<FwdMsg>) -> Flusher {
        let period = Duration::from_millis(period);
        let sleep = time::sleep(period);
        let sender = PollSender::new(send_channel.clone());
        Flusher {
            sleep,
            sender,
            period,
        }
    }

}

impl Future for Flusher {
    type Output = Never;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {

        let period = self.period;
        let mut this = self.project();

        if let Poll::Pending = this.sleep.as_mut().poll(cx) {
            return Poll::Pending;
        }
        trace!(target: "flusher", "flusher waking"); 
        if let Poll::Ready(res) = this.sender.poll_reserve(cx) {
            res.unwrap();
            trace!(target: "flusher", "sending flush message");
            this.sender.send_item(FwdMsg::Flush).unwrap();
            trace!("flush message sent, resetting sleep timer");
            this.sleep.reset(Instant::now() + period)
        }
        Poll::Pending
    }
}
