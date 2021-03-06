/*!
# logfwd

`logfwd` provides some utilities for forwarding logs received
over a UDP socket handed over from systemd via socket
activation to a TLS over TCP receiver, specifically built
for papertrail.
*/

pub mod clean_kill;
pub mod drano;
pub mod tls_send;
pub mod udp_recv;
pub mod error;
pub use error::Error;

type NothingError = Result<(), Box<dyn std::error::Error + Send + Sync>>;

/// Shutdown message
#[derive(Clone,Debug)]
pub struct Shutdown;

/**
`FwdMsg` is used for messaging between the sender and receiver half.
It is sent over a [tokio::sync::mpsc] bounded channel.

[tokio::sync::mpsc]: tokio::sync::mpsc
*/
#[derive(Debug)]
pub enum FwdMsg {
    /// owned boxed [u8] slice is the log msg to be forwarded
    Message(Box<[u8]>),
    /// Must be sent peridocially (use [Flusher][f]) to force the TLS connection to flush
    /// 
    /// [f]: crate::drano::Flusher
    Flush,
    /// Tells the TLS/TCP stream to shut down
    Close,
}