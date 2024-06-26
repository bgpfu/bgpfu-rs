use std::io::stderr;

use anyhow::{anyhow, Context};
use clap::Parser;
use clap_verbosity_flag::{Verbosity, WarnLevel};
use tracing_log::AsTrace as _;

use netconf::{
    message::rpc::operation::{Builder, Datastore, Filter, GetConfig, Opaque},
    transport::Password,
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
    let mut session = Session::ssh(addr, args.username, args.password)
        .await
        .context("failed to establish netconf session")?;
    println!(
        "negotiated protocol version {:?}",
        session.context().protocol_version()
    );
    println!("server capabilities:");
    session
        .context()
        .server_capabilities()
        .iter()
        .for_each(|capability| {
            println!("    {capability:?}");
        });
    let (config, _) = tokio::try_join!(
        session
            .rpc::<GetConfig<Opaque>, _>(|builder| builder
                .source(Datastore::Running)?
                .filter(Some(Filter::Subtree(
                    "<configuration><system/></configuration>".to_string()
                )))?
                .finish())
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

    #[arg(short, long, default_value_t = 830)]
    port: u16,

    #[arg(short, long, default_value = "test")]
    username: String,

    #[arg(short = 'P', long, default_value = "test123")]
    password: Password,

    #[command(flatten)]
    verbosity: Verbosity<WarnLevel>,
}
