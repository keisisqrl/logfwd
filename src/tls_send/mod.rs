use crate::FwdMsg;
mod tls;

use tokio_rustls::{TlsConnector};
use tokio::io::AsyncWriteExt;
use tokio_rustls::client::TlsStream;
use tokio::sync::mpsc::Receiver;
use tokio::net::TcpStream;
use std::string::String;

use std::error::Error;
use std::sync::Arc;
use rustls::client::ServerName;

use tls::get_client_config;
use tracing::{trace,debug};

pub struct Sender {
    send_stream: Option<TlsStream<TcpStream>>,
    recv_channel: Receiver<FwdMsg>,
    hostname: String,
    port: u16
}

impl Sender {
    pub fn new(hostname: String, port: u16, recv_half: Receiver<FwdMsg>) -> Sender {
        Sender {
            send_stream: None,
            recv_channel: recv_half,
            hostname,
            port
        }
    }

    pub async fn init(&mut self) -> Result<(),Box<dyn Error>> {
        let stream: TcpStream = TcpStream::connect((self.hostname.as_str(),self.port)).await?;
        trace!(target: "tls_sender_init", "TCP connected");

        let connector = TlsConnector::from(Arc::new(get_client_config()));

        trace!(target: "tls_sender_init", "TLS connector created");

        let server_name = ServerName::try_from(self.hostname.as_str())?;

        self.send_stream = Some(connector.connect(server_name,stream).await?);

        debug!(target: "tls_sender_init", "TLS stream connected");
        
        Ok(())
    }

    pub async fn run(&mut self) -> crate::NothingError {
        let send_stream = self.send_stream.as_mut().ok_or("Must init before run!")?;
        trace!(target: "tls_sender_run", "unwrapped send_stream");
        debug!(target: "tls_sender_run", "enter loop");
        loop {
            trace!(target: "tls_sender_run", "waiting for message");
            let incoming = self.recv_channel.recv().await;

            // trace!(target: "tls_sender_run", "channel returned: {:#?}", incoming);
            trace!(target: "tls_sender_run", "channel returned message");

            if incoming.is_none() {
                debug!(target: "tls_sender_run", "channel closed, leaving");
                break;
            }
            
            match incoming.unwrap() {
                FwdMsg::Flush => {
                    trace!(target: "tls_sender_run", "got flush message");
                    send_stream.flush().await?;
                    trace!(target: "tls_sender_run", "flush done");
                }
                FwdMsg::Message(bytes) => {
                    trace!(target: "tls_sender_run", "writing {} bytes", bytes.len());
                    let sent = send_stream.write(&bytes).await?;
                    trace!(target: "tls_sender_run", "wrote {} bytes", sent);
                }
                FwdMsg::Close => {
                    trace!(target: "tls_sender_run", "got close message, leaving");
                    break;
                }
            }
        }

        debug!("TLS sender shutting down");
        send_stream.shutdown().await?;
        debug!("TLS sender shutdown finished");

        Ok(())
    }
}