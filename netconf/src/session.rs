use std::{
    collections::{hash_map::Entry, HashMap},
    fmt::{self, Debug, Display},
    future::Future,
    mem,
    num::NonZeroU32,
    str::FromStr,
    sync::Arc,
};

use rustls_pki_types::{CertificateDer, InvalidDnsNameError, PrivateKeyDer, ServerName};
use tokio::{net::ToSocketAddrs, sync::Mutex};

use crate::{
    capabilities::{Base, Capabilities},
    message::{
        rpc::{
            self,
            operation::{Builder, CloseSession, ReplyData},
        },
        ClientHello, ClientMsg, ReadError, ServerHello, ServerMsg,
    },
    transport::{Password, Ssh, Tls, Transport},
    Error,
};

/// An identifier used by a NETCONF server to uniquely identify a session.
#[allow(clippy::module_name_repetitions)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SessionId(NonZeroU32);

impl SessionId {
    pub(crate) fn new(n: u32) -> Option<Self> {
        NonZeroU32::new(n).map(Self)
    }
}

impl FromStr for SessionId {
    type Err = ReadError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(Self(s.parse().map_err(Self::Err::SessionIdParse)?))
    }
}

impl Display for SessionId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        Display::fmt(&self.0, f)
    }
}

/// A NETCONF client session over a secure transport `T`.
///
/// [`Session`] instances provide direct access to asynchronous NETCONF protocol operations. The
/// library user is responsible for ensuring the correct ordering of operations to ensure, for
/// example, safe config modification. See [RFC6241] for additional guidance.
///
/// [RFC6241]: https://datatracker.ietf.org/doc/html/rfc6241#appendix-E
#[derive(Debug)]
pub struct Session<T: Transport> {
    transport_tx: Arc<Mutex<T::SendHandle>>,
    transport_rx: Arc<Mutex<T::RecvHandle>>,
    context: Context,
    last_message_id: rpc::MessageId,
    requests: Arc<Mutex<HashMap<rpc::MessageId, OutstandingRequest>>>,
}

/// NETCONF session state container.
#[derive(Debug)]
pub struct Context {
    session_id: SessionId,
    protocol_version: Base,
    client_capabilities: Capabilities,
    server_capabilities: Capabilities,
}

impl Context {
    const fn new(
        session_id: SessionId,
        protocol_version: Base,
        client_capabilities: Capabilities,
        server_capabilities: Capabilities,
    ) -> Self {
        Self {
            session_id,
            protocol_version,
            client_capabilities,
            server_capabilities,
        }
    }

    /// The NETCONF `session-id` of the current session.
    #[must_use]
    pub const fn session_id(&self) -> SessionId {
        self.session_id
    }

    /// The base NETCONF protocol version negotiated on the current session.
    #[must_use]
    pub const fn protocol_version(&self) -> Base {
        self.protocol_version
    }

    /// The set of NETCONF capabilities advertised by the client during `<hello>` message exchange.
    #[must_use]
    pub const fn client_capabilities(&self) -> &Capabilities {
        &self.client_capabilities
    }

    /// The set of NETCONF capabilities advertised by the client during `<hello>` message exchange.
    #[must_use]
    pub const fn server_capabilities(&self) -> &Capabilities {
        &self.server_capabilities
    }
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
    /// Establish a new NETCONF session over an SSH transport.
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
    /// Establish a new NETCONF session over a TLS transport.
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
        let client_hello = ClientHello::default();
        let (mut tx, mut rx) = transport.split();
        let ((), server_hello) =
            tokio::try_join!(client_hello.send(&mut tx), ServerHello::recv(&mut rx))?;
        let transport_tx = Arc::new(Mutex::new(tx));
        let transport_rx = Arc::new(Mutex::new(rx));
        let session_id = server_hello.session_id();
        let server_capabilities = server_hello.capabilities();
        let client_capabilities = client_hello.capabilities();
        let protocol_version = client_capabilities.highest_common_version(&server_capabilities)?;
        let context = Context::new(
            session_id,
            protocol_version,
            client_capabilities,
            server_capabilities,
        );
        let requests = Arc::new(Mutex::new(HashMap::default()));
        Ok(Self {
            transport_tx,
            transport_rx,
            context,
            requests,
            last_message_id: rpc::MessageId::default(),
        })
    }

    /// Get the session state [`Context`] of this session.
    #[must_use]
    pub const fn context(&self) -> &Context {
        &self.context
    }

    /// Execute a NETCONF RPC operation on the current session.
    ///
    /// See the [`rpc::operation`] module for available operations and their request builder APIs.
    ///
    /// RPC requests are built and validated against the [`Context`] of the current session - in
    /// particular, against the list of capabilities advertised by the NETCONF server in the
    /// `<hello>` message exchange.
    ///
    /// The `build_fn` closure must accept an instance of the operation request
    /// [`Builder`][rpc::Operation::Builder], configure the builder, and then convert it to a
    /// validated request by calling [`Builder::finish()`][rpc::operation::Builder::finish].
    ///
    /// This method returns a nested [`Future`], reflecting the fact that the request is sent to
    /// the NETCONF server asynchronously and then the response is later received asynchronously.
    ///
    /// The `Output` of both the outer and inner `Future` are of type `Result`.
    ///
    /// An [`Err`] variant returned by awaiting the outer future indicates either a request validation
    /// error or a session/transport error encountered while sending the RPC request.
    ///
    /// An [`Err`] variant returned by awaiting the inner future indicates either a
    /// session/transport error while receiving the `<rpc-reply>` message, an error parsing the
    /// received XML, or one-or-more application layer errors returned by the NETCONF server. The
    /// latter case may be identified by matching on the [`Error::RpcError`] variant.
    #[tracing::instrument(skip(self, build_fn))]
    pub async fn rpc<O, F>(
        &mut self,
        build_fn: F,
    ) -> Result<impl Future<Output = Result<<O::ReplyData as ReplyData>::Ok, Error>>, Error>
    where
        O: rpc::Operation,
        // TODO: consider whether F should be Fn or FnOnce
        F: Fn(O::Builder<'_>) -> Result<O, Error> + Send,
    {
        let message_id = self.last_message_id.increment();
        let request = O::new(&self.context, build_fn)
            .map(|operation| rpc::Request::new(message_id, operation))?;
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

    /// Close the NETCONF session gracefully using the `<close-session>` RPC operation.
    #[tracing::instrument(skip(self))]
    pub async fn close(mut self) -> Result<impl Future<Output = Result<(), Error>>, Error> {
        self.rpc::<CloseSession, _>(Builder::finish)
            .await
            .map(|fut| async move { fut.await.map(|()| drop(self)) })
    }
}
