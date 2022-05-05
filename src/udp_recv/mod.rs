use crate::Error;
use crate::FwdMsg;
use crate::Shutdown;

use futures_util::Stream;
use once_cell::sync::OnceCell;
use pin_project::pin_project;
use std::{
    future::Future,
    net::UdpSocket as StdUdp,
    os::unix::io::{FromRawFd, RawFd},
    pin::Pin,
    task::{Context, Poll},
};
use tokio::{
    io::ReadBuf,
    net::UdpSocket,
    sync::{broadcast, mpsc},
};
use tokio_stream::wrappers::BroadcastStream;
use tokio_util::sync::PollSender;
use tracing::debug;

#[pin_project]
pub struct Receiver {
    recv_socket: UdpSocket,
    send_channel: PollSender<FwdMsg>,
    #[pin]
    bcast_stream: BroadcastStream<Shutdown>,
    readbuf: ReadBuf<'static>,
}

fn alloc_buf(buf_len: usize) -> &'static mut Vec<u8> {
    static mut BUF: OnceCell<Vec<u8>> = OnceCell::new();
    unsafe {
        BUF.get_or_init(|| Vec::with_capacity(buf_len));
        BUF.get_mut().unwrap()
    }
}

impl Receiver {
    pub fn new(
        fd: RawFd,
        send_half: &mpsc::Sender<FwdMsg>,
        bcast_send: &broadcast::Sender<Shutdown>,
        buf_len: usize,
    ) -> Receiver {
        let std_socket: StdUdp = unsafe { StdUdp::from_raw_fd(fd) };
        // If you set systemd to give logfwd a nonblocking socket
        // (NOT default) this is unnecessary, but it's harmless
        // and good for safety. It took me ages to figure this
        // problem out.
        std_socket.set_nonblocking(true).unwrap();
        let tokio_socket: UdpSocket = UdpSocket::from_std(std_socket).unwrap();
        let static_vec = alloc_buf(buf_len);
        return Receiver {
            recv_socket: tokio_socket,
            send_channel: PollSender::new(send_half.clone()),
            bcast_stream: BroadcastStream::from(bcast_send.subscribe()),
            readbuf: ReadBuf::uninit(static_vec.spare_capacity_mut()),
        };
    }
}

impl Future for Receiver {
    type Output = Result<(), Error>;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let mut this = self.project();

        if let Poll::Ready(_) = this.bcast_stream.as_mut().poll_next(cx) {
            debug!("udp receiver shutting down");
            return Poll::Ready(Ok(()));
        }

        match this.send_channel.poll_reserve(cx) {
            Poll::Pending => {
                return Poll::Pending;
            }
            Poll::Ready(Err(_)) => return Poll::Ready(Err(Error::ChannelClosed)),
            _ => (),
        }

        match this.recv_socket.poll_recv(cx, &mut this.readbuf) {
            Poll::Ready(Ok(())) => {
                let msg = Vec::from(this.readbuf.filled());

                let msg = FwdMsg::Message(msg.into_boxed_slice());

                if let Err(_) = this.send_channel.send_item(msg) {
                    unreachable!();
                }

                if let Poll::Ready(_) = this.recv_socket.poll_recv_ready(cx) {
                    cx.waker().wake_by_ref();
                }

                Poll::Pending
            }

            Poll::Pending => Poll::Pending,

            Poll::Ready(Err(e)) => Poll::Ready(Err(Error::UDPSocketError(e))),
        }
    }
}
