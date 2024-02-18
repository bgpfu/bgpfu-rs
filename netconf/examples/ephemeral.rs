use std::{
    fs::File,
    io::{stderr, BufReader},
    path::{Path, PathBuf},
};

use anyhow::{anyhow, Context};
use clap::Parser;
use clap_verbosity_flag::{Verbosity, WarnLevel};
use rustls_pki_types::{CertificateDer, PrivateKeyDer};
use tracing_log::AsTrace;

use netconf::{
    message::rpc::operation::{
        edit_config::DefaultOperation, junos, Builder, Commit, Datastore, EditConfig, GetConfig,
        Opaque,
    },
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
    let ca_cert = read_cert(&args.ca_cert).context("failed to read CA certificate")?;
    let client_cert = read_cert(&args.client_cert).context("failed to read client certificate")?;
    let client_key =
        read_private_key(&args.client_key).context("failed to read client private key")?;
    let mut session = Session::tls(addr, args.server_name, ca_cert, client_cert, client_key)
        .await
        .context("failed to establish netconf session")?;
    // open & lock the ephemeral db
    let (_, existing_config) = tokio::try_join!(
        session
            .rpc::<junos::OpenConfiguration, _>(|builder| builder
                .ephemeral(Some("example"))
                .finish())
            .await?,
        session
            .rpc::<GetConfig<Opaque>, _>(|builder| builder.source(Datastore::Candidate)?.finish())
            .await?,
    )?;
    println!("current config:");
    println!("{existing_config}");
    let (_, candidate_config) = tokio::try_join!(
        session
            .rpc::<EditConfig<Opaque>, _>(|builder| builder
                .target(Datastore::Candidate)?
                .config(
                    r#"
                    <configuration>
                        <policy-options>
                            <policy-statement>
                                <name>bar</name>
                                <term>
                                    <name>inet</name>
                                    <from>
                                        <family>inet</family>
                                        <route-filter>
                                            <address>192.0.2.0/24</address>
                                            <prefix-length-range>/24-/30</prefix-length-range>
                                        </route-filter>
                                    </from>
                                    <then><accept/></then>
                                </term>
                            </policy-statement>
                        </policy-options>
                    </configuration>
                "#
                    .into()
                )
                .default_operation(DefaultOperation::Replace)
                .finish())
            .await?,
        session
            .rpc::<GetConfig<Opaque>, _>(|builder| builder.source(Datastore::Candidate)?.finish())
            .await?,
    )?;
    println!("candidate config:");
    println!("{candidate_config}");
    // commit, unlock & close the ephemeral db and close the session
    let (_, _, _) = tokio::try_join!(
        session.rpc::<Commit, _>(|builder| builder.finish()).await?,
        session
            .rpc::<junos::CloseConfiguration, _>(|builder| builder.finish())
            .await?,
        session.close().await?,
    )?;
    Ok(())
}

fn read_cert(path: &Path) -> anyhow::Result<CertificateDer<'static>> {
    File::open(path)
        .context("failed to open certificate file")
        .map(BufReader::new)
        .and_then(|ref mut reader| {
            match rustls_pemfile::read_one(reader).context("failed to read certificate file")? {
                Some(rustls_pemfile::Item::X509Certificate(cert)) => Ok(cert),
                Some(item) => Err(anyhow!("expected X.509 certificate, got {item:?}")),
                None => Err(anyhow!("no certificate found in file '{}'", path.display())),
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
