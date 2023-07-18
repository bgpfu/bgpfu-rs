use std::fs::File;
use std::path::PathBuf;

use anyhow::Context;

use bgpfu::RpslEvaluator;

use clap::Parser;

use clap_verbosity_flag::Verbosity;

use ip::traits::PrefixSet as _;

use rpsl::expr::MpFilterExpr;

use simplelog::WriteLogger;

use crate::jet::Transport;

/// Entry-point function for the `bgpfu-junos-agent`.
#[allow(clippy::missing_errors_doc)]
pub async fn main() -> anyhow::Result<()> {
    let args = Cli::parse();

    File::options()
        .create(true)
        .append(true)
        .open(&args.log_file)
        .context("failed to open log file '{args.log_file}'")
        .and_then(|file| {
            WriteLogger::init(
                args.verbosity.log_level_filter(),
                simplelog::Config::default(),
                file,
            )
            .context("failed to initialize logger")
        })?;
    log::debug!("started logging to {}", args.log_file.display());

    if let Some(expr) = args.filter_expr {
        log::debug!("attempting to evaluate RPSL expression '{expr}'");
        RpslEvaluator::new(&args.irrd_host, args.irrd_port)
            .context("failed to connect to IRRd server")?
            .evaluate(expr)
            .context("failed to evaluate RPSL mp-filter expression")?
            .ranges()
            .for_each(|range| log::info!("{range}"));
    } else {
        log::debug!("no RPSL expression provided");
    };

    let resp = if let Some(jet_host) = args.jet_host {
        Transport::https(
            jet_host,
            args.jet_port,
            args.ca_cert_path,
            args.tls_server_name,
        )?
    } else {
        Transport::unix(args.jet_sock)
    }
    .connect()
    .await?
    .authenticate(args.username, args.password)
    .await?
    .op_command("show version".to_string())
    .await?;

    println!("{resp}");

    Ok(())
}

/// A Junos extension application to manage IRR-based routing policy configuration.
#[derive(Debug, Parser)]
#[command(author, version, about, long_about = None)]
struct Cli {
    /// JET socket path
    #[arg(long, default_value = "/var/run/japi_jsd")]
    jet_sock: PathBuf,

    /// JET API endpoint hostname or IP address.
    #[arg(long)]
    jet_host: Option<String>,

    /// JET API endpoint port.
    #[arg(long, default_value_t = 32767)]
    jet_port: u16,

    /// JET API endpoint TLS CA certificate path.
    #[arg(long)]
    ca_cert_path: Option<PathBuf>,

    /// Override the domain name against which the server's TLS certificate is verified.
    #[arg(long)]
    tls_server_name: Option<String>,

    /// RPSL mp-filter expression to evaluate.
    #[arg(short, long)]
    filter_expr: Option<MpFilterExpr>,

    /// IRRd server hostname or IP address.
    #[arg(long, default_value = "whois.radb.net")]
    irrd_host: String,

    /// IRRd server port.
    #[arg(long, default_value_t = 43)]
    irrd_port: u16,

    /// Authentication username.
    #[arg(long, default_value = "bgpfu")]
    username: String,

    /// Authentication password.
    #[arg(long, default_value = "bgpFU1")]
    password: String,

    /// Path to log file.
    #[arg(long, default_value = "/var/log/bgpfu-junos-agent.log")]
    log_file: PathBuf,

    #[command(flatten)]
    verbosity: Verbosity,
}
