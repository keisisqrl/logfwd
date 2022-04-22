
pub mod udp_recv;
pub mod tls_send;
pub mod clean_kill;

pub type NothingError = Result<(), Box<dyn std::error::Error + Send + Sync>>; 

#[derive(Debug)]
pub enum FwdMsg {
    Message(Box<[u8]>),
    Flush,
    Close
}

pub mod drano {
    use tokio::sync::mpsc;
    use crate::{FwdMsg,NothingError};
    use tokio::time;
    use std::time::Duration;
    use log::trace;

    pub struct Flusher {
        duration: Duration,
        send_channel: mpsc::Sender<FwdMsg>
    }

    impl Flusher {
        pub fn new(duration: u64, send_channel: mpsc::Sender<FwdMsg>) -> Flusher {
            let duration = Duration::from_millis(duration);
            Flusher{
                duration,
                send_channel
            }
        }

        pub async fn run(&self) -> NothingError {
            let mut interval = time::interval(self.duration);

            loop {
                trace!(target:"flusher", "Waiting for interval");
                interval.tick().await;
                trace!(target:"flusher", "sending flush");
                self.send_channel.send(FwdMsg::Flush).await?;
            }
        }
    }
}