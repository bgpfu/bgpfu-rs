use std::{fmt::Debug, sync::Arc};

use async_trait::async_trait;
use bytes::{Bytes, BytesMut};
use memchr::memmem::Finder;
use rustls_pki_types::{CertificateDer, InvalidDnsNameError, PrivateKeyDer, ServerName};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt, ReadHalf, WriteHalf},
    net::{TcpStream, ToSocketAddrs},
};
use tokio_rustls::{
    client::TlsStream,
    rustls::{ClientConfig, RootCertStore},
    TlsConnector,
};

use crate::{message::MARKER, Error};

use super::{RecvHandle, SendHandle, Transport};

#[derive(Debug)]
pub struct Tls {
    stream: TlsStream<TcpStream>,
}

impl Tls {
    #[tracing::instrument]
    pub(crate) async fn connect<A, S>(
        addr: A,
        server_name: S,
        ca_cert: CertificateDer<'_>,
        client_cert: CertificateDer<'static>,
        client_key: PrivateKeyDer<'static>,
    ) -> Result<Self, Error>
    where
        A: ToSocketAddrs + Debug + Send,
        S: TryInto<ServerName<'static>, Error = InvalidDnsNameError> + Debug + Send,
    {
        let root_store = {
            let mut store = RootCertStore::empty();
            store.add(ca_cert)?;
            store
        };
        let config = Arc::new(
            ClientConfig::builder()
                .with_root_certificates(root_store)
                .with_client_auth_cert(vec![client_cert], client_key)?,
        );
        let domain = server_name.try_into()?;
        let tcp_stream = TcpStream::connect(addr).await?;
        let stream = TlsConnector::from(config)
            .connect(domain, tcp_stream)
            .await?;
        Ok(Self { stream })
    }
}

impl Transport for Tls {
    type SendHandle = Sender;
    type RecvHandle = Receiver;

    #[tracing::instrument(level = "debug")]
    fn split(self) -> (Self::SendHandle, Self::RecvHandle) {
        let (read, write) = tokio::io::split(self.stream);
        (Sender::new(write), Receiver::new(read))
    }
}

#[derive(Debug)]
pub struct Sender {
    write: WriteHalf<TlsStream<TcpStream>>,
}

impl Sender {
    const fn new(write: WriteHalf<TlsStream<TcpStream>>) -> Self {
        Self { write }
    }
}

#[async_trait]
impl SendHandle for Sender {
    #[tracing::instrument(level = "debug")]
    async fn send(&mut self, data: Bytes) -> Result<(), Error> {
        Ok(self.write.write_all(&data).await?)
    }
}

#[derive(Debug)]
pub struct Receiver {
    read: ReadHalf<TlsStream<TcpStream>>,
    buf: BytesMut,
    finder: Finder<'static>,
}

impl Receiver {
    fn new(read: ReadHalf<TlsStream<TcpStream>>) -> Self {
        let buf = BytesMut::with_capacity(2 << 20);
        let finder = Finder::new(MARKER);
        Self { read, buf, finder }
    }
}

#[async_trait]
impl RecvHandle for Receiver {
    #[tracing::instrument(level = "trace")]
    async fn recv(&mut self) -> Result<Bytes, Error> {
        loop {
            tracing::trace!("trying to read from transport");
            let len = self.read.read_buf(&mut self.buf).await?;
            tracing::trace!("read {len} bytes. buffer length is {}", self.buf.len());
            // TODO:
            // de-duplicate message-break searching with ssh transport
            tracing::trace!("searching for message-break marker");
            if let Some(index) = self.finder.find(&self.buf) {
                let end = index + MARKER.len();
                tracing::trace!("splitting {end} bytes from read buffer");
                let message = self.buf.split_to(end).freeze();
                tracing::trace!(?message);
                break Ok(message);
            }
        }
    }
}
