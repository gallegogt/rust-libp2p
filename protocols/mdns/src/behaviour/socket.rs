// Copyright 2018 Parity Technologies (UK) Ltd.
//
// Permission is hereby granted, free of charge, to any person obtaining a
// copy of this software and associated documentation files (the "Software"),
// to deal in the Software without restriction, including without limitation
// the rights to use, copy, modify, merge, publish, distribute, sublicense,
// and/or sell copies of the Software, and to permit persons to whom the
// Software is furnished to do so, subject to the following conditions:
//
// The above copyright notice and this permission notice shall be included in
// all copies or substantial portions of the Software.
//
// THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS
// OR IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
// FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
// AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
// LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING
// FROM, OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER
// DEALINGS IN THE SOFTWARE.

use std::{
    io::Error,
    net::{SocketAddr, UdpSocket},
    task::{Context, Poll},
};

/// Interface that must be implemented by the different runtimes to use the UdpSocket in async mode
pub trait AsyncSocket: Send + 'static {
    /// Create the async socket from the ```std::net::UdpSocket```
    fn from_socket(socket: UdpSocket) -> std::io::Result<Self>
    where
        Self: Sized;

    /// Attempts to receive a single packet on the socket from the remote address to which it is connected.
    fn poll_receive_packet(
        &mut self,
        _cx: &mut Context,
        _buf: &mut [u8],
    ) -> Poll<Result<Option<(usize, SocketAddr)>, Error>>;

    /// Attempts to send data on the socket to a given address.
    fn poll_send_packet(
        &mut self,
        _cx: &mut Context,
        _packet: &[u8],
        _to: SocketAddr,
    ) -> Poll<Result<(), Error>>;
}

#[cfg(feature = "async-io")]
pub mod asio {
    use super::*;
    use async_io::Async;
    use futures::FutureExt;

    /// AsyncIo UdpSocket
    pub type AsyncUdpSocket = Async<UdpSocket>;

    impl AsyncSocket for AsyncUdpSocket {
        /// Create the async socket from the ```std::net::UdpSocket```
        fn from_socket(socket: UdpSocket) -> std::io::Result<Self> {
            Async::new(socket)
        }

        /// Attempts to receive a single packet on the socket from the remote address to which it is connected.
        fn poll_receive_packet(
            &mut self,
            cx: &mut Context,
            buf: &mut [u8],
        ) -> Poll<Result<Option<(usize, SocketAddr)>, Error>> {
            // Poll receive socket.
            let _ = futures::ready!(self.poll_readable(cx));
            match self.recv_from(buf).now_or_never() {
                Some(Ok((len, from))) => Poll::Ready(Ok(Some((len, from)))),
                Some(Err(err)) => Poll::Ready(Err(err)),
                None => Poll::Ready(Ok(None)),
            }
        }

        /// Attempts to send data on the socket to a given address.
        fn poll_send_packet(
            &mut self,
            cx: &mut Context,
            packet: &[u8],
            to: SocketAddr,
        ) -> Poll<Result<(), Error>> {
            let _ = futures::ready!(self.poll_writable(cx));
            match self.send_to(packet, to).now_or_never() {
                Some(Ok(_)) => Poll::Ready(Ok(())),
                Some(Err(err)) => Poll::Ready(Err(err)),
                None => Poll::Pending,
            }
        }
    }
}

#[cfg(feature = "tokio")]
pub mod tokio {
    use super::*;
    use ::tokio::net::UdpSocket as TkUdpSocket;

    /// Tokio ASync Socket`
    pub type TokioUdpSocket = TkUdpSocket;

    impl AsyncSocket for TokioUdpSocket {
        /// Create the async socket from the ```std::net::UdpSocket```
        fn from_socket(socket: UdpSocket) -> std::io::Result<Self> {
            socket.set_nonblocking(true)?;
            TokioUdpSocket::from_std(socket)
        }

        /// Attempts to receive a single packet on the socket from the remote address to which it is connected.
        fn poll_receive_packet(
            &mut self,
            cx: &mut Context,
            buf: &mut [u8],
        ) -> Poll<Result<Option<(usize, SocketAddr)>, Error>> {
            match self.poll_recv_ready(cx) {
                Poll::Ready(Ok(_)) => match self.try_recv_from(buf) {
                    Ok((len, from)) => Poll::Ready(Ok(Some((len, from)))),
                    Err(err) if err.kind() == std::io::ErrorKind::WouldBlock => {
                        Poll::Ready(Ok(None))
                    }
                    Err(err) => Poll::Ready(Err(err)),
                },
                Poll::Ready(Err(e)) => Poll::Ready(Err(e)),
                _ => Poll::Pending,
            }
        }

        /// Attempts to send data on the socket to a given address.
        fn poll_send_packet(
            &mut self,
            cx: &mut Context,
            packet: &[u8],
            to: SocketAddr,
        ) -> Poll<Result<(), Error>> {
            match self.poll_send_ready(cx) {
                Poll::Ready(Ok(_)) => match self.try_send_to(packet, to) {
                    Ok(_len) => Poll::Ready(Ok(())),
                    Err(err) if err.kind() == std::io::ErrorKind::WouldBlock => Poll::Ready(Ok(())),
                    Err(err) => Poll::Ready(Err(err)),
                },
                Poll::Ready(Err(e)) => Poll::Ready(Err(e)),
                _ => Poll::Pending,
            }
        }
    }
}
