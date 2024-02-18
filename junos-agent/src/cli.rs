use std::num::NonZeroU64;
use std::path::PathBuf;
use std::str::FromStr;
use std::{fmt, path::Path};

use anyhow::{anyhow, Context};

use clap::{Args, Parser};

use clap_verbosity_flag::{Verbosity, WarnLevel};

use rustls_pki_types::ServerName;
use tracing_log::AsTrace;

use crate::task::Updater;

/// Entry-point function for `bgpfu-junos-agent`.
#[allow(clippy::missing_errors_doc)]
pub async fn main() -> anyhow::Result<()> {
    let args = Cli::parse();

    args.logging.init()?;

    let updater = Updater::new(args.netconf, args.irrd, args.junos);

    match args.frequency {
        Frequency::OneShot => updater.run().await,
        Frequency::Daemon(frequency) => updater.init_loop(frequency).start().await,
    }
}

/// A Junos extension application to manage IRR-based routing policy configuration.
#[derive(Debug, Parser)]
#[command(author, version, disable_help_subcommand = true)]
struct Cli {
    /// Frequency with which to update filter policies in daemon mode. Set to zero for one-shot
    /// mode.
    #[arg(short = 'f', long, default_value_t = 3600.into())]
    frequency: Frequency,

    #[command(flatten, next_help_heading = "Junos options")]
    junos: JunosOpts,

    #[command(flatten, next_help_heading = "NETCONF connection options")]
    netconf: NetconfOpts,

    #[command(flatten, next_help_heading = "IRR connection options")]
    irrd: IrrdOpts,

    #[command(flatten, next_help_heading = "Logging options")]
    logging: LoggingOpts,
}

#[derive(Debug, Clone, Copy)]
enum Frequency {
    /// Update filter policies once and then exit.
    OneShot,
    /// Run continuously, updating filter policies every `--frequency` seconds.
    Daemon(NonZeroU64),
}

impl From<u64> for Frequency {
    fn from(freq: u64) -> Self {
        freq.try_into().map_or(Self::OneShot, Self::Daemon)
    }
}

impl fmt::Display for Frequency {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::OneShot => 0.fmt(f),
            Self::Daemon(freq) => freq.fmt(f),
        }
    }
}

impl FromStr for Frequency {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        s.parse::<u64>()
            .context("failed to parse frequency")
            .map(Self::from)
    }
}

#[derive(Debug, Args)]
pub(super) struct NetconfOpts {
    /// NETCONF server hostname or IP address.
    #[arg(long = "netconf-host", id = "netconf-host", value_name = "HOST")]
    host: String,

    /// NETCONF server port.
    #[arg(
        long = "netconf-port",
        id = "netconf-port",
        default_value_t = 6513,
        value_name = "PORT"
    )]
    port: u16,

    /// NETCONF TLS transport CA certificate path.
    #[arg(long)]
    ca_cert_path: PathBuf,

    /// NETCONF TLS transport client certificate path.
    #[arg(long)]
    client_cert_path: PathBuf,

    /// NETCONF TLS transport client private key path.
    #[arg(long)]
    client_key_path: PathBuf,

    /// Override the domain name against which the NETCONF server's TLS certificate is verified.
    #[arg(long, value_parser = parse_server_name)]
    tls_server_name: Option<ServerName<'static>>,
}

fn parse_server_name(name: &str) -> anyhow::Result<ServerName<'static>> {
    let parsed = ServerName::try_from(name).context("failed to parse server name")?;
    Ok(parsed.to_owned())
}

impl NetconfOpts {
    pub(super) fn host(&self) -> &str {
        &self.host
    }

    pub(super) const fn port(&self) -> u16 {
        self.port
    }

    pub(super) fn ca_cert_path(&self) -> &Path {
        &self.ca_cert_path
    }

    pub(super) fn client_cert_path(&self) -> &Path {
        &self.client_cert_path
    }

    pub(super) fn client_key_path(&self) -> &Path {
        &self.client_key_path
    }

    pub(super) fn tls_server_name(&self) -> Option<ServerName<'static>> {
        self.tls_server_name.clone()
    }
}

#[derive(Debug, Args)]
struct LoggingOpts {
    /// Logging output destination
    #[arg(short = 'l', long, default_value = "/var/log/bgpfu-junos-agent.log")]
    logging_dest: LoggingDest,

    #[command(flatten)]
    verbosity: Verbosity<WarnLevel>,
}

impl LoggingOpts {
    fn init(self) -> anyhow::Result<()> {
        let level = self.verbosity.log_level_filter().as_trace();
        // TODO:
        // let (writer, _guard) = match self.logging_dest {
        //     LoggingDest::File(ref path) => {
        //         let canonical = path.canonicalize()?;
        //         let writer = tracing_appender::rolling::daily(
        //             canonical.parent().unwrap(),
        //             canonical.file_name().unwrap(),
        //         );
        //         tracing_appender::non_blocking(writer)
        //     }
        //     LoggingDest::StdErr => tracing_appender::non_blocking(std::io::stderr()),
        // };
        tracing_subscriber::fmt()
            .with_max_level(level)
            // .with_writer(writer)
            .with_writer(std::io::stderr)
            .try_init()
            .map_err(|err| anyhow!(err))
    }
}

#[derive(Debug, Clone)]
enum LoggingDest {
    StdErr,
    File(PathBuf),
}

impl fmt::Display for LoggingDest {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::StdErr => write!(f, "STDERR"),
            Self::File(path) => path.to_string_lossy().fmt(f),
        }
    }
}

impl FromStr for LoggingDest {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        if s == "STDERR" {
            Ok(Self::StdErr)
        } else {
            Ok(Self::File(s.into()))
        }
    }
}

#[derive(Debug, Args)]
pub(super) struct IrrdOpts {
    /// IRRd server hostname or IP address.
    #[arg(
        long = "irrd-host",
        id = "irrd-host",
        default_value = "whois.radb.net",
        value_name = "HOST"
    )]
    host: String,

    /// IRRd server port.
    #[arg(
        long = "irrd-port",
        id = "irrd-port",
        default_value_t = 43,
        value_name = "PORT"
    )]
    port: u16,
}

impl IrrdOpts {
    pub(super) fn host(&self) -> &str {
        &self.host
    }

    pub(super) const fn port(&self) -> u16 {
        self.port
    }
}

#[derive(Debug, Args)]
pub(super) struct JunosOpts {
    /// Junos ephemeral DB instance name.
    #[arg(long, default_value = "bgpfu")]
    ephemeral_db: String,
}

impl JunosOpts {
    pub(super) fn ephemeral_db(&self) -> &str {
        &self.ephemeral_db
    }
}
