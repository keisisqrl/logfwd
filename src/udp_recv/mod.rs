use crate::FwdMsg;

use std::os::unix::io::{FromRawFd, RawFd};
use std::net::UdpSocket as StdUdp;
use tokio::net::UdpSocket;
use tokio::sync::mpsc;

use log::{debug,trace};

pub struct Receiver {
    recv_socket: UdpSocket,
    send_channel: mpsc::Sender<FwdMsg>
}

impl Receiver {
    pub fn new(fd: RawFd, send_half: mpsc::Sender<FwdMsg>) -> Receiver {
        let std_socket: StdUdp = unsafe { StdUdp::from_raw_fd(fd) };
        std_socket.set_nonblocking(true).unwrap();
        let tokio_socket: UdpSocket = UdpSocket::from_std(std_socket).unwrap();
        return Receiver {
            recv_socket: tokio_socket,
            send_channel: send_half
        }
    }

    pub async fn run(&self) -> crate::NothingError {
        debug!(target: "udp_receiver_run", "entering loop");
        loop {
            let mut buf: Vec<u8> = vec![0;1500];

            trace!(target: "udp_receiver_run", "waiting for datagram, should yield");
            let len: usize = self.recv_socket.recv(&mut buf).await?;

            buf.truncate(len);

            trace!(target: "udp_receiver_run", "received {} bytes, truncated buf to {}", len, buf.len());

            let to_send: Box<[u8]> = buf.into_boxed_slice();

            let to_send: FwdMsg = FwdMsg::Message(to_send);

            trace!(target: "udp_receiver_run", "sending message {:?} to channel", to_send);
            self.send_channel.send(to_send).await?;
            trace!(target: "udp_receiver_run", "send finished");
        }
    }
}