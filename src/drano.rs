use crate::FwdMsg;
use futures_util::Future;
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

#[pin_project]
pub struct Flusher {
    #[pin]
    sleep: Sleep,
    #[pin]
    sender: PollSender<FwdMsg>,
    duration: Duration,
}

impl Flusher {
    pub fn new(duration: u64, send_channel: &Sender<FwdMsg>) -> Flusher {
        let duration = Duration::from_millis(duration);
        let sleep = time::sleep(duration);
        let sender = PollSender::new(send_channel.clone());
        Flusher {
            sleep,
            sender,
            duration,
        }
    }

}

impl Future for Flusher {
    type Output = ();

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {

        let duration = self.duration;
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
            this.sleep.reset(Instant::now() + duration)
        }
        Poll::Pending
    }
}
