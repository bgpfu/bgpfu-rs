use std::fmt::Debug;

use async_trait::async_trait;
use bytes::{Bytes, BytesMut};
use memchr::memmem::Finder;
use russh::{
    client::{connect, Config},
    ChannelMsg,
};
use russh_keys::key::PublicKey;
use tokio::{net::ToSocketAddrs, sync::mpsc, task::JoinHandle};

use super::{Password, RecvHandle, SendHandle, Transport};
use crate::{message::MARKER, Error};

#[derive(Debug)]
pub struct Ssh {
    _task: JoinHandle<Result<(), Error>>,
    in_queue: Receiver,
    out_queue: Sender,
}

impl Ssh {
    #[tracing::instrument]
    pub(crate) async fn connect<A>(
        addr: A,
        username: String,
        password: Password,
    ) -> Result<Self, Error>
    where
        A: ToSocketAddrs + Debug + Send,
    {
        tracing::info!("attempting to establish SSH session");
        let config = Config::default().into();
        let handler = Handler::new();
        let session = {
            let mut session = connect(config, addr, handler).await?;
            tracing::info!("ssh session established");
            if !session
                .authenticate_password(username.clone(), password.into_inner())
                .await?
            {
                return Err(Error::Authentication(username));
            };
            tracing::info!("ssh authentication sucessful");
            session
        };
        tracing::info!("attempting to open ssh channel");
        let mut channel = session.channel_open_session().await?;
        tracing::info!("ssh channel opened");
        tracing::info!("requesting netconf ssh subsystem");
        channel.request_subsystem(true, "netconf").await?;
        tracing::info!("netconf ssh subsystem activated");
        let (out_queue_tx, mut out_queue_rx) = mpsc::channel::<Bytes>(32);
        let (in_queue_tx, in_queue_rx) = mpsc::channel(32);
        let out_queue = Sender {
            inner: out_queue_tx,
        };
        let in_queue = Receiver { inner: in_queue_rx };
        let task = tokio::spawn(async move {
            let mut in_buf = BytesMut::new();
            let message_break = Finder::new(MARKER);
            loop {
                tokio::select! {
                    to_send = out_queue_rx.recv() => {
                        tracing::debug!("attempting to send message");
                        tracing::trace!(?to_send);
                        // TODO:
                        // we should probably handle this error?
                        if let Some(data) = to_send {
                            channel.data(data.as_ref()).await?;
                        } else {
                            break;
                        };
                        tracing::trace!("message sent");
                    }
                    msg = channel.wait() => {
                        if let Some(msg) = msg {
                            tracing::debug!("processing received msg");
                            tracing::trace!(?msg);
                            match msg {
                                ChannelMsg::Data{ data } => {
                                    tracing::debug!("got data on channel");
                                    in_buf.extend_from_slice(&data);
                                    tracing::debug!("checking for message break marker");
                                    if let Some(index) = message_break.find(&in_buf) {
                                        let end  = index + MARKER.len();
                                        tracing::info!("splitting {end} message bytes from input buffer");
                                        let message = in_buf.split_to(end).freeze();
                                        in_queue_tx.send(message).await?;
                                        tracing::debug!("message data enqueued sucessfully");
                                    };
                                }
                                ChannelMsg::Eof => {
                                    tracing::info!("got eof, hanging up");
                                    break;
                                }
                                _ => {
                                    tracing::debug!("ignoring msg {msg:?}");
                                }
                            }
                        } else {
                            // TODO: what should we do if it's None?
                        }
                    }
                }
            }
            Ok(())
        });
        Ok(Self {
            _task: task,
            in_queue,
            out_queue,
        })
    }
}

impl Transport for Ssh {
    type SendHandle = Sender;
    type RecvHandle = Receiver;

    #[tracing::instrument(level = "debug")]
    fn split(self) -> (Self::SendHandle, Self::RecvHandle) {
        (self.out_queue, self.in_queue)
    }
}

#[derive(Debug)]
pub struct Sender {
    inner: mpsc::Sender<Bytes>,
}

#[async_trait]
impl SendHandle for Sender {
    #[tracing::instrument(level = "trace")]
    async fn send(&mut self, data: Bytes) -> Result<(), Error> {
        Ok(self.inner.send(data).await?)
    }
}

#[derive(Debug)]
pub struct Receiver {
    inner: mpsc::Receiver<Bytes>,
}

#[async_trait]
impl RecvHandle for Receiver {
    #[tracing::instrument(level = "trace")]
    async fn recv(&mut self) -> Result<Bytes, Error> {
        self.inner
            .recv()
            .await
            .ok_or(Error::DequeueMessage("input message channel closed"))
    }
}

#[derive(Debug)]
struct Handler {}

impl Handler {
    const fn new() -> Self {
        Self {}
    }
}

#[async_trait]
impl russh::client::Handler for Handler {
    type Error = Error;

    // TODO
    #[tracing::instrument(skip_all)]
    async fn check_server_key(self, _: &PublicKey) -> Result<(Self, bool), Self::Error> {
        tracing::info!("NOT checking server public key");
        Ok((self, true))
    }
}
