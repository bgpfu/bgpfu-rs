use std::{fmt::Debug, future::Future, marker::PhantomData, sync::Arc};

use anyhow::{anyhow, Context};
use futures::TryFutureExt;
use netconf::{
    message::rpc::operation::{
        junos::{
            load_configuration::{Config, Merge, Xml},
            CloseConfiguration, CommitConfiguration, LoadConfiguration, OpenConfiguration,
        },
        Builder, Filter, GetConfig,
    },
    transport::{JunosLocal, Tls, Transport},
    Session,
};
use rustls_pki_types::ServerName;

use crate::{
    cli::NetconfTlsOpts,
    policies::{Fetch, Load},
};

mod pem;
use self::pem::{read_cert, read_private_key};

pub(crate) trait Target: Debug + Clone + Sized + Send {
    type Transport: Transport;

    fn connect(self) -> impl Future<Output = anyhow::Result<Client<Self, Closed>>> + Send;
}

#[derive(Debug, Clone, Copy)]
pub(crate) struct Local;

impl Target for Local {
    type Transport = JunosLocal;

    #[tracing::instrument(skip_all, level = "debug")]
    async fn connect(self) -> anyhow::Result<Client<Self, Closed>> {
        Session::junos_local()
            .await
            .context("failed to establish NETCONF session")
            .map(|session| Client {
                session,
                _db_state: PhantomData,
            })
    }
}

#[derive(Debug, Clone)]
pub(crate) struct Remote {
    opts: Arc<NetconfTlsOpts>,
}

impl Remote {
    pub(crate) fn new(opts: NetconfTlsOpts) -> Self {
        Self {
            opts: Arc::new(opts),
        }
    }
}

impl Target for Remote {
    type Transport = Tls;

    #[tracing::instrument(skip_all, level = "debug")]
    async fn connect(self) -> anyhow::Result<Client<Self, Closed>> {
        let (host, port) = (self.opts.host(), self.opts.port());
        tracing::debug!("trying to connect to NETCONF server at '{host}:{port}'");
        let addr = (host, port);
        let (ca_cert_path, client_cert_path, client_key_path) = (
            self.opts.ca_cert_path(),
            self.opts.client_cert_path(),
            self.opts.client_key_path(),
        );
        tracing::debug!(?ca_cert_path, ?client_cert_path, ?client_key_path);
        let (ca_cert, client_cert, client_key) = tokio::try_join!(
            read_cert(ca_cert_path),
            read_cert(client_cert_path),
            read_private_key(client_key_path)
        )?;
        let server_name = match self.opts.tls_server_name() {
            Some(name) => name,
            None => ServerName::try_from(host)?.to_owned(),
        };
        Session::tls(addr, server_name, ca_cert, client_cert, client_key)
            .await
            .context("failed to establish NETCONF session")
            .map(|session| Client {
                session,
                _db_state: PhantomData,
            })
    }
}

#[derive(Debug)]
pub(crate) enum Closed {}

#[derive(Debug)]
pub(crate) enum Open {}

#[derive(Debug)]
pub(crate) struct Client<T: Target, S> {
    session: Session<T::Transport>,
    _db_state: PhantomData<S>,
}

impl<T: Target> Client<T, Closed> {
    #[tracing::instrument(skip(self), level = "debug")]
    pub(crate) async fn open_db(mut self, name: &str) -> anyhow::Result<Client<T, Open>> {
        tracing::debug!("trying to open ephemeral database");
        self.session
            .rpc::<OpenConfiguration, _>(|builder| builder.ephemeral(Some(name)).finish())
            .await
            .context("failed to send NETCONF '<open-configration>' RPC request")?
            .await
            .context("failed to open ephemeral database")?;
        Ok(Client {
            session: self.session,
            _db_state: PhantomData,
        })
    }

    #[tracing::instrument(skip(self), level = "debug")]
    pub(crate) async fn close(self) -> anyhow::Result<()> {
        tracing::debug!("closing NETCONF session");
        self.session
            .close()
            .await
            .context("failed to send NETCONF '<close-session>' RPC request")?
            .await
            .context("error while closing NETCONF session")
    }
}

impl<T: Target> Client<T, Open> {
    #[tracing::instrument(skip(self), level = "debug")]
    pub(crate) async fn fetch_config<R>(
        &mut self,
    ) -> anyhow::Result<impl Future<Output = anyhow::Result<R>>>
    where
        R: Fetch,
    {
        tracing::debug!("trying to fetch configuration");
        let future = self
            .session
            .rpc::<GetConfig<R>, _>(|builder| {
                builder
                    .source(R::DATASTORE)?
                    .filter(R::FILTER.map(|filter| Filter::Subtree(filter.to_string())))?
                    .finish()
            })
            .await
            .context("failed to send NETCONF '<get-config>' RPC request")?
            .map_err(|err| anyhow!(err).context("failed to fetch configuration"));
        Ok(future)
    }

    #[tracing::instrument(skip(self, config), level = "debug")]
    pub(crate) async fn load_config<C>(&mut self, config: C) -> anyhow::Result<&mut Self>
    where
        C: Load,
    {
        tracing::debug!("trying to load candidate configuration");
        tracing::trace!(?config);
        let updates = {
            let mut updates = Vec::new();
            for update in config.updates() {
                updates.push(
                    self.session
                        .rpc::<LoadConfiguration<_>, _>(|builder| {
                            builder.source(Config::new(update, Xml, Merge)).finish()
                        })
                        .await
                        .context("failed to send NETCONF <load-configuration> RPC request")?,
                );
            }
            updates
        };
        for update in updates {
            update.await.context("failed to load configuration batch")?;
        }
        Ok(self)
    }

    #[allow(clippy::redundant_closure_for_method_calls)]
    #[tracing::instrument(skip(self), level = "debug")]
    pub(crate) async fn commit_config(&mut self) -> anyhow::Result<()> {
        tracing::debug!("trying to commit candidate configuration");
        self.session
            .rpc::<CommitConfiguration, _>(|builder| builder.finish())
            .await
            .context("failed to send NETCONF '<commit-configuration>' RPC request")?
            .await
            .context("failed to commit candidate configuration")
    }

    #[allow(clippy::redundant_closure_for_method_calls)]
    #[tracing::instrument(skip(self), level = "debug")]
    pub(crate) async fn close_db(mut self) -> anyhow::Result<Client<T, Closed>> {
        tracing::debug!("trying to close candidate configuration");
        self.session
            .rpc::<CloseConfiguration, _>(|builder| builder.finish())
            .await
            .context("failed to send NETCONF '<close-configration>' RPC request")?
            .await
            .context("failed to close ephemeral database")?;
        Ok(Client {
            session: self.session,
            _db_state: PhantomData,
        })
    }
}
