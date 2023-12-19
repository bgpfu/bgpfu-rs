use std::{
    fs::File,
    io::BufReader,
    path::{Path, PathBuf},
};

use anyhow::Context;
use clap::Parser;
use clap_verbosity_flag::{Verbosity, WarnLevel};
use rustls_pki_types::{CertificateDer, PrivateKeyDer};
use simplelog::{ColorChoice, TermLogger, TerminalMode};

use netconf::{
    message::rpc::operation::{Builder, Datastore, GetConfig},
    Session,
};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let args = Cli::parse();
    TermLogger::init(
        args.verbosity.log_level_filter(),
        Default::default(),
        TerminalMode::Stderr,
        ColorChoice::Auto,
    )
    .context("failed to init logger")?;
    let addr = (args.host.as_str(), args.port);
    let ca_cert = read_cert(&args.ca_cert).context("failed to read CA certificate")?;
    let client_cert = read_cert(&args.client_cert).context("failed to read client certificate")?;
    let client_key =
        read_private_key(&args.client_key).context("failed to read client private key")?;
    let mut session = Session::tls(addr, args.server_name, ca_cert, client_cert, client_key)
        .await
        .context("failed to establish netconf session")?;
    let (config, _) = tokio::try_join!(
        session
            .rpc::<GetConfig, _>(|builder| builder.source(Datastore::Running).finish())
            .await?,
        session.close().await?
    )?;
    println!("{config}");
    Ok(())
}

fn read_cert(path: &Path) -> anyhow::Result<CertificateDer<'static>> {
    File::open(path)
        .context("failed to open certificate file")
        .map(BufReader::new)
        .and_then(|ref mut reader| {
            match rustls_pemfile::read_one(reader).context("failed to read certificate file")? {
                Some(rustls_pemfile::Item::X509Certificate(cert)) => Ok(cert),
                Some(item) => anyhow::bail!("expected X.509 certificate, got {item:?}"),
                None => anyhow::bail!("no certificate found in file '{}'", path.display()),
            }
        })
}

fn read_private_key(path: &Path) -> anyhow::Result<PrivateKeyDer<'static>> {
    File::open(path)
        .context("failed to open private key file")
        .map(BufReader::new)
        .and_then(|ref mut reader| {
            rustls_pemfile::private_key(reader).context("failed to read private key file")
        })?
        .ok_or_else(|| anyhow::anyhow!("no private key found in file '{}'", path.display()))
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
