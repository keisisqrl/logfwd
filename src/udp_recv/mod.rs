use crate::FwdMsg;
use crate::Shutdown;

use std::{
    net::UdpSocket as StdUdp,
    os::unix::io::{FromRawFd, RawFd},
};
use std::future::Future;
use std::pin::Pin;
use std::task::{Context, Poll};
use tokio::{net::UdpSocket, sync::mpsc};
use tokio::sync::broadcast;
use tokio::io::ReadBuf;
use tokio_util::sync::PollSender;
use crate::Error;

pub struct Receiver {
    recv_socket: UdpSocket,
    send_channel: PollSender<FwdMsg>,
    bcast_listen: broadcast::Receiver<Shutdown>
}

impl Receiver {
    pub fn new(fd: RawFd, send_half: &mpsc::Sender<FwdMsg>, bcast_send: &broadcast::Sender<Shutdown>) -> Receiver {
        let std_socket: StdUdp = unsafe { StdUdp::from_raw_fd(fd) };
        // If you set systemd to give logfwd a nonblocking socket
        // (NOT default) this is unnecessary, but it's harmless
        // and good for safety. It took me ages to figure this
        // problem out.
        std_socket.set_nonblocking(true).unwrap();
        let tokio_socket: UdpSocket = UdpSocket::from_std(std_socket).unwrap();
        return Receiver {
            recv_socket: tokio_socket,
            send_channel: PollSender::new(send_half.clone()),
            bcast_listen: bcast_send.subscribe()
        };
    }

}

impl Future for Receiver {
    type Output = Result<(),Error>;

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {

        match self.bcast_listen.try_recv() {
            Err(broadcast::error::TryRecvError::Empty) => {}
            _ => {
                return Poll::Ready(Ok(()));
            }
        }
        
        match self.send_channel.poll_reserve(cx) {
            Poll::Pending => { return Poll::Pending;}
            Poll::Ready(Err(_)) => {
                return Poll::Ready(Err(Error::ChannelClosed))
            }
            _ => ()
        }

        let mut buf: Vec<u8> = vec![0;9000];
        let mut readbuf = ReadBuf::new(&mut buf);

        match self.recv_socket.poll_recv(cx,&mut readbuf) {
            Poll::Ready(Ok(())) => {
                let msg = Vec::from(readbuf.filled());

                let msg = FwdMsg::Message(
                    msg.into_boxed_slice()
                );

                if let Err(_) = self.send_channel.send_item(msg) {
                    unreachable!();
                } 

                if let Poll::Ready(_) = self.recv_socket.poll_recv_ready(cx) {
                    cx.waker().wake_by_ref();
                }
                Poll::Pending
            }

            Poll::Pending => Poll::Pending,

            Poll::Ready(Err(e)) => {
                Poll::Ready(Err(Error::UDPSocketError(e)))
            }
        }
    }
}