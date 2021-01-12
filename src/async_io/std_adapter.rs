use crate::async_io::runtime::{AsyncRead, AsyncWrite};
use std::{
    io::{self, Read, Write},
    marker::Unpin,
    pin::Pin,
    task::{Context, Poll},
};

#[derive(Debug)]
pub(crate) struct StdAdapter<S> {
    pub(crate) inner: S,
    pub(crate) context: *mut (),
}

// *mut () context is neither Send nor Sync
unsafe impl<S: Send> Send for StdAdapter<S> {}
unsafe impl<S: Sync> Sync for StdAdapter<S> {}

impl<S> StdAdapter<S>
where
    S: Unpin,
{
    pub(crate) fn with_context<F, R>(&mut self, f: F) -> R
    where
        F: FnOnce(&mut Context<'_>, Pin<&mut S>) -> R,
    {
        unsafe {
            assert!(!self.context.is_null());
            let waker = &mut *(self.context as *mut _);
            f(waker, Pin::new(&mut self.inner))
        }
    }
}

impl<S> Read for StdAdapter<S>
where
    S: AsyncRead + Unpin,
{
    #[cfg(feature = "io-async-std")]
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        match self.with_context(|ctx, stream| stream.poll_read(ctx, buf)) {
            Poll::Ready(r) => r,
            Poll::Pending => Err(io::Error::from(io::ErrorKind::WouldBlock)),
        }
    }

    #[cfg(feature = "io-tokio")]
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        let mut buf = tokio::io::ReadBuf::new(buf);
        match self.with_context(|ctx, stream| stream.poll_read(ctx, &mut buf)) {
            Poll::Ready(r) => r.map(|_| buf.filled().len()),
            Poll::Pending => Err(io::Error::from(io::ErrorKind::WouldBlock)),
        }
    }
}

impl<S> Write for StdAdapter<S>
where
    S: AsyncWrite + Unpin,
{
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        match self.with_context(|ctx, stream| stream.poll_write(ctx, buf)) {
            Poll::Ready(r) => r,
            Poll::Pending => Err(io::Error::from(io::ErrorKind::WouldBlock)),
        }
    }

    fn flush(&mut self) -> io::Result<()> {
        match self.with_context(|ctx, stream| stream.poll_flush(ctx)) {
            Poll::Ready(r) => r,
            Poll::Pending => Err(io::Error::from(io::ErrorKind::WouldBlock)),
        }
    }
}
