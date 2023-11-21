use std::{
    collections::{hash_map::Entry, HashMap},
    fmt::Debug,
    future::Future,
    mem,
    sync::Arc,
};

use tokio::{net::ToSocketAddrs, sync::Mutex};

use crate::{
    message::{
        rpc, Capabilities, Capability, ClientHello, ClientMsg, ServerHello, ServerMsg, BASE,
    },
    transport::{Password, Ssh, Transport},
    Error,
};

#[derive(Debug)]
pub struct Session<T: Transport> {
    transport_tx: Arc<Mutex<T::SendHandle>>,
    transport_rx: Arc<Mutex<T::RecvHandle>>,
    capabilities: Capabilities,
    session_id: usize,
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
        let transport = Ssh::connect(addr, username, password).await?;
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
}

impl<T: Transport> Session<T> {
    #[must_use]
    pub const fn session_id(&self) -> usize {
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
        let request = rpc::Request::<O>::new(message_id, operation);
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
}
