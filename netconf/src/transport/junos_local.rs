use std::{fmt::Debug, io, process::Stdio, sync::Arc};

use async_trait::async_trait;
use bytes::{Bytes, BytesMut};
use memchr::memmem::Finder;
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    process::{Child, ChildStdin, ChildStdout, Command},
};

use crate::{message::MARKER, Error};

use super::{RecvHandle, SendHandle, Transport};

const CLI_PATH: &str = "/usr/sbin/cli";
const CLI_ARGS: &[&str] = &["xml-mode", "netconf", "need-trailer"];

// TODO:
// figure out what to do with stderr
#[derive(Debug)]
pub struct JunosLocal {
    handle: Arc<Child>,
    stdin: ChildStdin,
    stdout: ChildStdout,
}

impl JunosLocal {
    #[tracing::instrument(skip_all, level = "debug")]
    pub(crate) async fn connect() -> Result<Self, Error> {
        let mut child = Command::new(CLI_PATH)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .args(CLI_ARGS)
            .kill_on_drop(true)
            .spawn()?;
        let stdout = child
            .stdout
            .take()
            .ok_or_else(|| io::Error::other("failed to handle for child stdin"))?;
        let stdin = child
            .stdin
            .take()
            .ok_or_else(|| io::Error::other("failed to handle for child stdin"))?;
        let handle = Arc::new(child);
        Ok(Self {
            handle,
            stdin,
            stdout,
        })
    }
}

impl Transport for JunosLocal {
    type SendHandle = Sender;
    type RecvHandle = Receiver;

    #[tracing::instrument(level = "debug")]
    fn split(self) -> (Self::SendHandle, Self::RecvHandle) {
        (
            Sender::new(self.handle.clone(), self.stdin),
            Receiver::new(self.handle, self.stdout),
        )
    }
}

#[derive(Debug)]
pub struct Sender {
    _handle: Arc<Child>,
    write: ChildStdin,
}

impl Sender {
    const fn new(handle: Arc<Child>, write: ChildStdin) -> Self {
        Self {
            _handle: handle,
            write,
        }
    }
}

#[async_trait]
impl SendHandle for Sender {
    #[tracing::instrument(level = "debug")]
    async fn send(&mut self, data: Bytes) -> Result<(), Error> {
        self.write.write_all(&data).await?;
        self.write.flush().await?;
        Ok(())
    }
}

#[derive(Debug)]
pub struct Receiver {
    _handle: Arc<Child>,
    read: ChildStdout,
    buf: BytesMut,
    finder: Finder<'static>,
}

impl Receiver {
    fn new(handle: Arc<Child>, read: ChildStdout) -> Self {
        let buf = BytesMut::with_capacity(1 << 10);
        let finder = Finder::new(MARKER);
        Self {
            _handle: handle,
            read,
            buf,
            finder,
        }
    }
}

#[async_trait]
impl RecvHandle for Receiver {
    #[tracing::instrument(skip(self), level = "debug")]
    async fn recv(&mut self) -> Result<Bytes, Error> {
        // TODO:
        // handle case when read ends part way through an end marker
        let mut searched = 0;
        loop {
            tracing::trace!(?self.buf, "searching for message-break marker");
            if let Some(index) = self.finder.find(&self.buf[searched..]) {
                let end = searched + index + MARKER.len();
                tracing::debug!("splitting {end} bytes from read buffer");
                let message = self.buf.split_to(end).freeze();
                tracing::trace!(?message);
                break Ok(message);
            }
            searched = self.buf.len();
            tracing::trace!("trying to read from transport");
            let len = self.read.read_buf(&mut self.buf).await?;
            tracing::trace!("read {len} bytes. buffer length is {}", self.buf.len());
        }
    }
}
