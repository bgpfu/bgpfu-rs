use anyhow::Context;
use clap::Parser;
use clap_verbosity_flag::{Verbosity, WarnLevel};
use simplelog::{ColorChoice, TermLogger, TerminalMode};

use netconf::{
    message::rpc::operation::get_config::{GetConfig, Source},
    transport::Password,
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
    let mut session = Session::ssh(addr, args.username, args.password)
        .await
        .context("failed to establish netconf session")?;
    println!("negotiated capabilities:");
    session.capabilities().for_each(|capability| {
        println!("    {}", capability.uri());
    });
    let (config, _) = tokio::try_join!(
        session
            .rpc(GetConfig::new(
                Source::Running,
                Some("<configuration><system/></configuration>".to_string())
            ))
            .await?,
        session.close().await?
    )?;
    if let Some(config) = config {
        println!("{config}");
    } else {
        anyhow::bail!("expected config data")
    };
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
