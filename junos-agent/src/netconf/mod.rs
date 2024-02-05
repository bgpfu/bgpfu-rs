use std::{fmt::Debug, marker::PhantomData, path::Path};

use anyhow::Context;

use netconf::{
    message::rpc::operation::{
        edit_config::DefaultOperation,
        junos::{CloseConfiguration, OpenConfiguration},
        Builder, Commit, Datastore, EditConfig, Filter, GetConfig,
    },
    transport::Tls,
    Session,
};

use rustls_pki_types::ServerName;

use crate::config::{read::ReadConfig, write::WriteConfig};

mod pem;
use self::pem::{read_cert, read_private_key};

trait DbState {}

#[derive(Debug)]
pub(crate) enum Closed {}
impl DbState for Closed {}

#[derive(Debug)]
pub(crate) enum Open {}
impl DbState for Open {}

#[derive(Debug)]
pub(crate) struct Client<S> {
    session: Session<Tls>,
    _db_state: PhantomData<S>,
}

impl Client<Closed> {
    #[tracing::instrument]
    pub(crate) async fn connect(
        host: &str,
        port: u16,
        ca_cert_path: &Path,
        client_cert_path: &Path,
        client_key_path: &Path,
        server_name: Option<ServerName<'static>>,
    ) -> anyhow::Result<Self> {
        let addr = (host, port);
        let (ca_cert, client_cert, client_key) = tokio::try_join!(
            read_cert(ca_cert_path),
            read_cert(client_cert_path),
            read_private_key(client_key_path)
        )?;
        let server_name = match server_name {
            Some(name) => name,
            None => ServerName::try_from(host)?.to_owned(),
        };
        Session::tls(addr, server_name, ca_cert, client_cert, client_key)
            .await
            .context("failed to establish NETCONF session")
            .map(|session| Self {
                session,
                _db_state: PhantomData,
            })
    }

    #[tracing::instrument(level = "debug")]
    pub(crate) async fn open_db(mut self, name: &str) -> anyhow::Result<Client<Open>> {
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

    #[tracing::instrument(level = "debug")]
    pub(crate) async fn close(self) -> anyhow::Result<()> {
        self.session
            .close()
            .await
            .context("failed to send NETCONF '<close-session>' RPC request")?
            .await
            .context("error while closing NETCONF session")
    }
}

impl Client<Open> {
    #[tracing::instrument(level = "debug")]
    pub(crate) async fn get_config<T>(&mut self) -> anyhow::Result<T>
    where
        T: ReadConfig,
    {
        self.session
            .rpc::<GetConfig<T>, _>(|builder| {
                builder
                    .source(Datastore::Running)?
                    .filter(Some(Filter::Subtree(T::FILTER.to_string())))?
                    .finish()
            })
            .await
            .context("failed to send NETCONF '<get-config>' RPC request")?
            .await
            .context("failed to retreive running configuration")
    }

    #[tracing::instrument(skip(config), level = "debug")]
    pub(crate) async fn load_config<T>(&mut self, config: T) -> anyhow::Result<&mut Self>
    where
        T: WriteConfig,
    {
        tracing::trace!(?config);
        self.session
            .rpc::<EditConfig<T>, _>(|builder| {
                builder
                    .target(Datastore::Candidate)?
                    .config(config)
                    .default_operation(DefaultOperation::Replace)
                    .finish()
            })
            .await
            .context("failed to send NETCONF <edit-config> RPC request")?
            .await
            .context("failed to replace candidate configuration")?;
        Ok(self)
    }

    #[allow(clippy::redundant_closure_for_method_calls)]
    #[tracing::instrument(level = "debug")]
    pub(crate) async fn commit_config(&mut self) -> anyhow::Result<()> {
        self.session
            .rpc::<Commit, _>(|builder| builder.finish())
            .await
            .context("failed to send NETCONF '<commit>' RPC request")?
            .await
            .context("failed to commit candidate configuration")
    }

    #[allow(clippy::redundant_closure_for_method_calls)]
    #[tracing::instrument(level = "debug")]
    pub(crate) async fn close_db(mut self) -> anyhow::Result<Client<Closed>> {
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
