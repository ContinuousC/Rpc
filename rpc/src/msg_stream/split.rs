/******************************************************************************
 * Copyright ContinuousC. Licensed under the "Elastic License 2.0".           *
 ******************************************************************************/

use tokio::{
    io::{AsyncRead, AsyncWrite},
    net::{TcpStream, UnixStream},
};

pub trait Split: AsyncRead + AsyncWrite + Send + Sync + Unpin {
    type ReadStream: AsyncRead + Send + Sync + Unpin;
    type WriteStream: AsyncWrite + Send + Sync + Unpin;
    fn split(self) -> (Self::ReadStream, Self::WriteStream);
    fn unsplit(read: Self::ReadStream, write: Self::WriteStream) -> Self;
}

// pub struct Splittable<S>(pub S);

impl Split for UnixStream {
    type ReadStream = tokio::net::unix::OwnedReadHalf;
    type WriteStream = tokio::net::unix::OwnedWriteHalf;
    fn split(self) -> (Self::ReadStream, Self::WriteStream) {
        UnixStream::into_split(self)
    }
    fn unsplit(read: Self::ReadStream, write: Self::WriteStream) -> Self {
        read.reunite(write).unwrap()
    }
}

impl Split for TcpStream {
    type ReadStream = tokio::net::tcp::OwnedReadHalf;
    type WriteStream = tokio::net::tcp::OwnedWriteHalf;
    fn split(self) -> (Self::ReadStream, Self::WriteStream) {
        TcpStream::into_split(self)
    }
    fn unsplit(read: Self::ReadStream, write: Self::WriteStream) -> Self {
        read.reunite(write).unwrap()
    }
}

impl<S: AsyncRead + AsyncWrite + Send + Sync + Unpin> Split
    for tokio_rustls::server::TlsStream<S>
{
    type ReadStream = tokio::io::ReadHalf<Self>;
    type WriteStream = tokio::io::WriteHalf<Self>;
    fn split(self) -> (Self::ReadStream, Self::WriteStream) {
        tokio::io::split(self)
    }
    fn unsplit(read: Self::ReadStream, write: Self::WriteStream) -> Self {
        read.unsplit(write)
    }
}

impl<S: AsyncRead + AsyncWrite + Send + Sync + Unpin> Split
    for tokio_rustls::client::TlsStream<S>
{
    type ReadStream = tokio::io::ReadHalf<Self>;
    type WriteStream = tokio::io::WriteHalf<Self>;
    fn split(self) -> (Self::ReadStream, Self::WriteStream) {
        tokio::io::split(self)
    }
    fn unsplit(read: Self::ReadStream, write: Self::WriteStream) -> Self {
        read.unsplit(write)
    }
}

// impl<S: AsyncRead + AsyncWrite> Split for Splittable<S> {
//     type ReadStream = tokio::io::ReadHalf<S>;
//     type WriteStream = tokio::io::WriteHalf<S>;
//     fn split(self) -> (Self::ReadStream, Self::WriteStream) {
//         tokio::io::split(self.0)
//     }
// }

// impl<S: AsyncRead> AsyncRead for Splittable<S> {
//     fn poll_read(
//         self: std::pin::Pin<&mut Self>,
//         cx: &mut std::task::Context<'_>,
//         buf: &mut tokio::io::ReadBuf<'_>,
//     ) -> std::task::Poll<std::io::Result<()>> {
//         self.0.poll_read(std::pin::Pin::new(&mut self.0), cx, buf)
//     }
// }

// impl<S: AsyncWrite> AsyncWrite for Splittable<S> {
//     fn poll_write(
//         self: std::pin::Pin<&mut Self>,
//         cx: &mut std::task::Context<'_>,
//         buf: &[u8],
//     ) -> std::task::Poll<Result<usize, std::io::Error>> {
//         self.0.poll_write(std::pin::Pin::new(&mut self.0), cx, buf)
//     }

//     fn poll_flush(
//         self: std::pin::Pin<&mut Self>,
//         cx: &mut std::task::Context<'_>,
//     ) -> std::task::Poll<Result<(), std::io::Error>> {
//         self.0.poll_flush(std::pin::Pin::new(&mut self.0), cx)
//     }

//     fn poll_shutdown(
//         self: std::pin::Pin<&mut Self>,
//         cx: &mut std::task::Context<'_>,
//     ) -> std::task::Poll<Result<(), std::io::Error>> {
//         self.0.poll_shutdown(std::pin::Pin::new(&mut self.0), cx)
//     }
// }
