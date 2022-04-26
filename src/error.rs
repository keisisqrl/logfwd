
#[derive(Debug)]
pub enum Error {
    ChannelClosed,
    SinkDisconnected,
    UDPSocketError(std::io::Error),
    TlsSocketError(std::io::Error)
}