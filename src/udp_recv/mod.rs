use crate::FwdMsg;
use crate::Shutdown;

use std::{
    net::UdpSocket as StdUdp,
    os::unix::io::{FromRawFd, RawFd},
};
use std::future::Future;
use std::pin::Pin;
use std::task::{Context, Poll};
use tokio::{net::UdpSocket, sync::{mpsc, broadcast}};
use tokio::io::ReadBuf;
use tokio_stream::wrappers::BroadcastStream;
use tokio_util::sync::PollSender;
use futures_util::Stream;
use crate::Error;
use pin_project::pin_project;
use tracing::debug;

#[pin_project]
pub struct Receiver {
    recv_socket: UdpSocket,
    send_channel: PollSender<FwdMsg>,
    #[pin]
    bcast_stream: BroadcastStream<Shutdown>,
    buf_vec: Vec<u8>
}

impl Receiver {
    pub fn new(fd: RawFd, send_half: &mpsc::Sender<FwdMsg>, bcast_send: &broadcast::Sender<Shutdown>, buf_len: usize) -> Receiver {
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
            bcast_stream: BroadcastStream::from(bcast_send.subscribe()),
            buf_vec: Vec::with_capacity(buf_len)
        };
    }

}

impl Future for Receiver {
    type Output = Result<(),Error>;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {

        let mut this = self.project();

        if let Poll::Ready(_) = this.bcast_stream.as_mut().poll_next(cx) {
            debug!("udp receiver shutting down");
            return Poll::Ready(Ok(()));
        }
        
        match this.send_channel.poll_reserve(cx) {
            Poll::Pending => { return Poll::Pending;}
            Poll::Ready(Err(_)) => {
                return Poll::Ready(Err(Error::ChannelClosed))
            }
            _ => ()
        }

        let mut readbuf = ReadBuf::uninit(this.buf_vec.spare_capacity_mut());

        match this.recv_socket.poll_recv(cx,&mut readbuf) {
            Poll::Ready(Ok(())) => {
                let msg = Vec::from(readbuf.filled());

                let msg = FwdMsg::Message(
                    msg.into_boxed_slice()
                );

                if let Err(_) = this.send_channel.send_item(msg) {
                    unreachable!();
                } 

                if let Poll::Ready(_) = this.recv_socket.poll_recv_ready(cx) {
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