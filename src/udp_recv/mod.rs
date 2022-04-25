use crate::FwdMsg;

use std::{
    net::UdpSocket as StdUdp,
    os::unix::io::{FromRawFd, RawFd},
};
use std::future::Future;
use std::pin::Pin;
use std::task::{Context, Poll};
use tokio::{net::UdpSocket, sync::mpsc};
use tokio::io::ReadBuf;
use tokio_util::sync::PollSender;

pub struct Receiver {
    recv_socket: UdpSocket,
    send_channel: PollSender<FwdMsg>,
}

impl Receiver {
    pub fn new(fd: RawFd, send_half: &mpsc::Sender<FwdMsg>) -> Receiver {
        let std_socket: StdUdp = unsafe { StdUdp::from_raw_fd(fd) };
        // If you set systemd to give logfwd a nonblocking socket
        // (NOT default) this is unnecessary, but it's harmless
        // and good for safety. It took me ages to figure this
        // problem out.
        std_socket.set_nonblocking(true).unwrap();
        let tokio_socket: UdpSocket = UdpSocket::from_std(std_socket).unwrap();
        return Receiver {
            recv_socket: tokio_socket,
            send_channel: PollSender::new(send_half.clone())
        };
    }

}

impl Future for Receiver {
    type Output = crate::NothingError;

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        
        if let Poll::Pending = self.send_channel.poll_reserve(cx) {
            return Poll::Pending;
        }

        let mut buf: Vec<u8> = vec![0;9000];
        let mut readbuf = ReadBuf::new(&mut buf);

        match self.recv_socket.poll_recv(cx,&mut readbuf) {
            Poll::Ready(Ok(())) => {
                let msg = Vec::from(readbuf.filled());

                let msg = FwdMsg::Message(
                    msg.into_boxed_slice()
                );

                if let Err(e) = self.send_channel.send_item(msg) {
                    return Poll::Ready(Err(Box::new(e)));
                } 

                if let Poll::Ready(_) = self.recv_socket.poll_recv_ready(cx) {
                    cx.waker().wake_by_ref();
                }
                Poll::Pending
            }

            Poll::Pending => Poll::Pending,

            Poll::Ready(Err(e)) => {
                Poll::Ready(Err(Box::new(e)))
            }
        }
    }
}