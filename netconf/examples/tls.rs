use std::{io::stderr, path::PathBuf};

use anyhow::{anyhow, Context};
use clap::Parser;
use clap_verbosity_flag::{Verbosity, WarnLevel};
use rustls_pki_types::{pem::PemObject, CertificateDer, PrivateKeyDer};
use tracing_log::AsTrace as _;

use netconf::{
    message::rpc::operation::{Builder, Datastore, GetConfig, Opaque},
    Session,
};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let args = Cli::parse();
    tracing_subscriber::fmt()
        .with_max_level(args.verbosity.log_level_filter().as_trace())
        .with_writer(stderr)
        .try_init()
        .map_err(|err| anyhow!(err))?;
    let addr = (args.host.as_str(), args.port);
    let ca_cert =
        CertificateDer::from_pem_file(&args.ca_cert).context("failed to read CA certificate")?;
    let client_cert = CertificateDer::from_pem_file(&args.client_cert)
        .context("failed to read client certificate")?;
    let client_key = PrivateKeyDer::from_pem_file(&args.client_key)
        .context("failed to read client private key")?;
    let mut session = Session::tls(addr, args.server_name, ca_cert, client_cert, client_key)
        .await
        .context("failed to establish netconf session")?;
    let (config, _) = tokio::try_join!(
        session
            .rpc::<GetConfig<Opaque>, _>(|builder| builder.source(Datastore::Running)?.finish())
            .await?,
        session.close().await?
    )?;
    println!("{config}");
    Ok(())
}

#[derive(Debug, Parser)]
#[command(author, version)]
struct Cli {
    host: String,

    #[arg(short, long, default_value_t = 6513)]
    port: u16,

    #[arg(long, default_value = "cr2-lab")]
    server_name: String,

    #[arg(long, default_value = "netconf/examples/pki/ca.crt")]
    ca_cert: PathBuf,

    #[arg(long, default_value = "netconf/examples/pki/client.crt")]
    client_cert: PathBuf,

    #[arg(long, default_value = "netconf/examples/pki/client.key")]
    client_key: PathBuf,

    #[command(flatten)]
    verbosity: Verbosity<WarnLevel>,
}
