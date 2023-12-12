use std::{
    collections::{hash_map::Entry, HashMap},
    fmt::Debug,
    future::Future,
    mem,
    num::NonZeroU32,
    str::FromStr,
    sync::Arc,
};

use rustls_pki_types::{CertificateDer, InvalidDnsNameError, PrivateKeyDer, ServerName};
use tokio::{net::ToSocketAddrs, sync::Mutex};

use crate::{
    message::{
        rpc::{self, operation::CloseSession},
        Capabilities, Capability, ClientHello, ClientMsg, ServerHello, ServerMsg, BASE,
    },
    transport::{Password, Ssh, Tls, Transport},
    Error,
};

#[allow(clippy::module_name_repetitions)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SessionId(NonZeroU32);

impl SessionId {
    pub(crate) fn new(n: u32) -> Option<Self> {
        NonZeroU32::new(n).map(Self)
    }
}

impl FromStr for SessionId {
    type Err = Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(Self(s.parse()?))
    }
}

#[derive(Debug)]
pub struct Session<T: Transport> {
    transport_tx: Arc<Mutex<T::SendHandle>>,
    transport_rx: Arc<Mutex<T::RecvHandle>>,
    capabilities: Capabilities,
    session_id: SessionId,
    last_message_id: rpc::MessageId,
    requests: Arc<Mutex<HashMap<rpc::MessageId, OutstandingRequest>>>,
}

#[derive(Debug)]
enum OutstandingRequest {
    Pending,
    Ready(rpc::PartialReply),
    Complete,
}

impl OutstandingRequest {
    #[tracing::instrument(level = "debug")]
    fn take(&mut self) -> Result<Option<rpc::PartialReply>, Error> {
        match mem::replace(self, Self::Complete) {
            mut pending @ Self::Pending => {
                mem::swap(self, &mut pending);
                Ok(None)
            }
            Self::Complete => Err(Error::RequestComplete),
            Self::Ready(reply) => Ok(Some(reply)),
        }
    }
}

impl Session<Ssh> {
    #[tracing::instrument]
    pub async fn ssh<A>(addr: A, username: String, password: Password) -> Result<Self, Error>
    where
        A: ToSocketAddrs + Send + Debug,
    {
        tracing::info!("starting ssh transport");
        let transport = Ssh::connect(addr, username, password).await?;
        Self::new(transport).await
    }
}

impl Session<Tls> {
    #[tracing::instrument]
    pub async fn tls<A, S>(
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
        tracing::info!("starting tls transport");
        let transport = Tls::connect(addr, server_name, ca_cert, client_cert, client_key).await?;
        Self::new(transport).await
    }
}

impl<T: Transport> Session<T> {
    async fn new(transport: T) -> Result<Self, Error> {
        let client_hello = ClientHello::new(&[BASE]);
        let (mut tx, mut rx) = transport.split();
        let ((), server_hello) =
            tokio::try_join!(client_hello.send(&mut tx), ServerHello::recv(&mut rx))?;
        let transport_tx = Arc::new(Mutex::new(tx));
        let transport_rx = Arc::new(Mutex::new(rx));
        let capabilities = client_hello.common_capabilities(&server_hello)?;
        let session_id = server_hello.session_id();
        let requests = Arc::new(Mutex::new(HashMap::default()));
        Ok(Self {
            transport_tx,
            transport_rx,
            capabilities,
            session_id,
            requests,
            last_message_id: rpc::MessageId::default(),
        })
    }

    #[must_use]
    pub const fn session_id(&self) -> SessionId {
        self.session_id
    }

    pub fn capabilities(&self) -> impl Iterator<Item = &Capability> {
        self.capabilities.iter()
    }

    #[tracing::instrument(skip(self))]
    pub async fn rpc<O: rpc::Operation>(
        &mut self,
        operation: O,
    ) -> Result<impl Future<Output = Result<Option<O::ReplyData>, Error>>, Error> {
        let message_id = self.last_message_id.increment();
        let request = rpc::Request::new(message_id, operation);
        #[allow(clippy::significant_drop_in_scrutinee)]
        match self.requests.lock().await.entry(message_id) {
            Entry::Occupied(_) => return Err(Error::MessageIdCollision(message_id)),
            Entry::Vacant(entry) => {
                request.send(&mut *self.transport_tx.lock().await).await?;
                _ = entry.insert(OutstandingRequest::Pending);
            }
        };
        let requests = self.requests.clone();
        let rx = self.transport_rx.clone();
        let fut = async move {
            loop {
                if let Some(partial) = requests
                    .lock()
                    .await
                    .get_mut(&message_id)
                    .ok_or_else(|| Error::RequestNotFound(message_id))?
                    .take()?
                {
                    let reply: rpc::Reply<O> = partial.try_into()?;
                    break reply.into_result();
                };
                let reply = rpc::PartialReply::recv(&mut *rx.lock().await).await?;
                #[allow(clippy::significant_drop_in_scrutinee)]
                match requests
                    .lock()
                    .await
                    .get_mut(&reply.message_id())
                    .ok_or_else(|| Error::RequestNotFound(reply.message_id()))?
                {
                    OutstandingRequest::Complete => break Err(Error::RequestComplete),
                    OutstandingRequest::Ready(_) => {
                        break Err(Error::MessageIdCollision(reply.message_id()))
                    }
                    pending @ OutstandingRequest::Pending => {
                        _ = mem::replace(pending, OutstandingRequest::Ready(reply));
                    }
                }
            }
        };
        Ok(fut)
    }

    #[tracing::instrument(skip(self))]
    pub async fn close(mut self) -> Result<impl Future<Output = Result<(), Error>>, Error> {
        self.rpc(CloseSession)
            .await
            .map(|fut| async move { fut.await.map(|_| drop(self)) })
    }
}
