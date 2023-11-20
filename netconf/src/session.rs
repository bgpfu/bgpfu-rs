use std::{
    collections::{hash_map::Entry, HashMap},
    fmt::Debug,
    future::Future,
    mem,
};

use bytes::Bytes;
use tokio::net::ToSocketAddrs;

use crate::{
    message::{
        rpc, Capabilities, Capability, ClientHello, ClientMsg, ServerHello, ServerMsg, BASE,
    },
    transport::{Password, RecvHandle, SendHandle, Ssh, Transport},
    Error,
};

#[derive(Debug)]
pub struct Session<T> {
    transport: T,
    capabilities: Capabilities,
    session_id: usize,
    last_message_id: rpc::MessageId,
    requests: HashMap<rpc::MessageId, OutstandingRequest>,
}

#[derive(Debug)]
enum OutstandingRequest {
    Pending,
    Ready(rpc::PartialReply),
    Complete,
}

impl OutstandingRequest {
    #[tracing::instrument]
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
        let mut transport = Ssh::connect(addr, username, password).await?;
        let client_hello = ClientHello::new(&[BASE]);
        let (tx, rx) = transport.split();
        let ((), server_hello) = tokio::try_join!(client_hello.send(tx), ServerHello::recv(rx))?;
        let capabilities = client_hello.common_capabilities(&server_hello)?;
        let session_id = server_hello.session_id();
        Ok(Self {
            transport,
            capabilities,
            session_id,
            last_message_id: rpc::MessageId::default(),
            requests: HashMap::default(),
        })
    }
}

impl<T: Transport> Session<T> {
    pub const fn session_id(&self) -> usize {
        self.session_id
    }

    pub fn capabilities(&self) -> impl Iterator<Item = &Capability> {
        self.capabilities.iter()
    }

    #[tracing::instrument(skip(self))]
    pub async fn rpc<O: rpc::Operation>(
        &mut self,
        request: O::RequestData,
    ) -> Result<impl Future<Output = Result<Option<O::ReplyData>, Error>> + '_, Error> {
        let message_id = self.last_message_id.increment();
        let request = rpc::Request::<O>::new(message_id, request);
        let (tx, rx) = self.transport.split();
        match self.requests.entry(message_id) {
            Entry::Occupied(_) => return Err(Error::MessageIdCollision(message_id)),
            Entry::Vacant(entry) => {
                request.send(tx).await?;
                _ = entry.insert(OutstandingRequest::Pending);
            }
        };
        let requests = &mut self.requests;
        let fut = async move {
            loop {
                if let Some(partial) = requests
                    .get_mut(&message_id)
                    .ok_or_else(|| Error::RequestNotFound(message_id))?
                    .take()?
                {
                    let reply: rpc::Reply<O> = partial.try_into()?;
                    break reply.into_result();
                };
                let reply = rpc::PartialReply::recv(rx).await?;
                match requests
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
    pub async fn send(&mut self, data: Bytes) -> Result<(), Error> {
        tracing::debug!("trying to write to transport");
        self.transport.send(data).await
    }

    #[tracing::instrument(skip(self))]
    pub async fn recv(&mut self) -> Result<Bytes, Error> {
        tracing::debug!("trying to read from transport");
        self.transport.recv().await
    }
}
