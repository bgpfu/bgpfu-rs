use std::fmt;
use std::fs::File;
use std::num::NonZeroU64;
use std::path::PathBuf;
use std::str::FromStr;

use anyhow::Context;

use clap::{Args, Parser, Subcommand};

use clap_verbosity_flag::{Verbosity, WarnLevel};

use simplelog::{SimpleLogger, WriteLogger};

use crate::{jet::Transport, task::Updater};

/// Entry-point function for `bgpfu-junos-agent`.
#[allow(clippy::missing_errors_doc)]
pub async fn main() -> anyhow::Result<()> {
    let args = Cli::parse();

    args.logging.init()?;

    let updater = Updater::new(
        args.jet_transport.init()?,
        args.junos.username,
        args.junos.password,
        args.junos.ephemeral_db,
        args.irrd.host,
        args.irrd.port,
    );

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

    #[command(subcommand)]
    jet_transport: JetTransport,

    #[command(flatten, next_help_heading = "Junos options")]
    junos: JunosOpts,

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

// TODO: this would be better as sets of mutually exclusive args
// see https://github.com/clap-rs/clap/issues/2621
#[derive(Debug, Subcommand)]
enum JetTransport {
    Local {
        /// JET socket path
        #[arg(long, default_value = "/var/run/japi_jsd")]
        jet_sock: PathBuf,
    },
    Remote {
        /// JET API endpoint hostname or IP address.
        #[arg(long)]
        jet_host: String,

        /// JET API endpoint port.
        #[arg(long, default_value_t = 32767)]
        jet_port: u16,

        /// JET API endpoint TLS CA certificate path.
        #[arg(long)]
        ca_cert_path: Option<PathBuf>,

        /// Override the domain name against which the server's TLS certificate is verified.
        #[arg(long)]
        tls_server_name: Option<String>,
    },
}

impl JetTransport {
    fn init(self) -> anyhow::Result<Transport> {
        match self {
            Self::Local { jet_sock } => Ok(Transport::unix(jet_sock)),
            Self::Remote {
                jet_host,
                jet_port,
                ca_cert_path,
                tls_server_name,
            } => Transport::https(jet_host, jet_port, ca_cert_path, tls_server_name),
        }
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
        let level = self.verbosity.log_level_filter();
        let config = simplelog::Config::default();
        match self.logging_dest {
            LoggingDest::File(ref path) => File::options()
                .create(true)
                .append(true)
                .open(path)
                .context("failed to open log file '{path.display()}'")
                .and_then(|file| {
                    WriteLogger::init(level, config, file).context("failed to initialize logger")
                }),
            LoggingDest::StdErr => {
                SimpleLogger::init(level, config).context("failed to initialize logger")
            }
        }
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
struct IrrdOpts {
    /// IRRd server hostname or IP address.
    #[arg(long = "irrd-host", default_value = "whois.radb.net")]
    host: String,

    /// IRRd server port.
    #[arg(long = "irrd-port", default_value_t = 43)]
    port: u16,
}

#[derive(Debug, Args)]
struct JunosOpts {
    /// Junos ephemeral DB instance name.
    #[arg(long, default_value = "bgpfu")]
    ephemeral_db: String,

    /// Authentication username.
    #[arg(short = 'u', long, default_value = "bgpfu")]
    username: String,

    /// Authentication password.
    #[arg(short = 'p', long, default_value = "bgpFU1")]
    password: String,
}
