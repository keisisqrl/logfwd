
pub mod udp_recv;
pub mod tls_send;
pub mod clean_kill;
pub mod drano;

pub type NothingError = Result<(), Box<dyn std::error::Error + Send + Sync>>; 

#[derive(Debug)]
pub enum FwdMsg {
    Message(Box<[u8]>),
    Flush,
    Close
}