use tokio::signal::unix::{SignalKind,signal};
use tokio::sync::mpsc::Sender;
use tokio::task;
use crate::FwdMsg;
use libsystemd::daemon;

use std::error::Error;
use std::fmt;
use log::{trace,debug,info};

#[derive(Debug)]
struct Exiting;

impl fmt::Display for Exiting {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Exiting due to signal")
    }
}

impl Error for Exiting {} 

pub async fn kill_handler(send_task: task::JoinHandle<crate::NothingError>, channel: Sender<FwdMsg>) 
    -> crate::NothingError {
    let mut stream = signal(SignalKind::terminate())?;
    
    debug!(target:"clean_kill", "waiting for sigterm");

    stream.recv().await;

    info!(target: "clean_kill", "sigterm received, shutting down");

    trace!(target: "clean_kill", "notifying systemd of stopping status");

    daemon::notify(false,&[daemon::NotifyState::Stopping]).unwrap();

    trace!(target: "clean_kill", "systemd notified, telling tls sender to close");

    channel.send(FwdMsg::Close).await?;

    trace!(target: "clean_kill", "sent msg to tls sender, waiting for it to finish");

    send_task.await??;

    debug!(target: "clean_kill", "signal handler finished");

    Ok(())
}
