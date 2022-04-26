//! # Drano
//! 
//! Contains the future [Flusher].

use crate::{FwdMsg, Shutdown, Error};
use futures_util::{Stream, Future};
use log::{trace,debug};
use std::{
    pin::Pin,
    task::{Context, Poll},
    time::Duration,
};
use tokio::{
    sync::{mpsc::Sender, broadcast},
    time::{self, Instant, Sleep},
};
use tokio_util::sync::PollSender;
use tokio_stream::wrappers::BroadcastStream;
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
    #[pin]
    bcast_stream: BroadcastStream<Shutdown>
}

impl Flusher {

    /// Create a new [Flusher] future.
    /// 
    /// Takes a `period` in milliseconds and a ref to a [Sender].
    /// 
    /// Run the returned Future with [tokio::spawn]
    pub fn new(period: u64, send_channel: &Sender<FwdMsg>, bcast_send: &broadcast::Sender<Shutdown>) -> Flusher {
        let period = Duration::from_millis(period);
        let sleep = time::sleep(period);
        let sender = PollSender::new(send_channel.clone());
        Flusher {
            sleep,
            sender,
            period,
            bcast_stream: BroadcastStream::from(bcast_send.subscribe())
        }
    }

}

impl Future for Flusher {
    type Output = Result<(),Error>;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {

        let period = self.period;
        let mut this = self.project();

        if let Poll::Ready(_) = this.bcast_stream.as_mut().poll_next(cx) {
            debug!("flusher shutting down");
            return Poll::Ready(Ok(()));
        }

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
