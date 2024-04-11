use std::fmt::Display;
use std::num::NonZeroU64;
use std::path::PathBuf;
use std::str::FromStr;
use std::{fmt, path::Path};

use anyhow::{anyhow, Context};

use clap::{Args, Parser, Subcommand};

use clap_verbosity_flag::{InfoLevel, Verbosity};

use rolling_file::{BasicRollingFileAppender, RollingConditionBasic};
use rustls_pki_types::ServerName;
use tracing_appender::non_blocking::{NonBlocking, NonBlockingBuilder, WorkerGuard};
use tracing_log::AsTrace;
use tracing_subscriber::EnvFilter;
use ubyte::ByteUnit;

use crate::{
    netconf::{Local, Remote},
    task::Updater,
};

/// Entry-point function for `bgpfu-junos-agent`.
#[allow(clippy::missing_errors_doc)]
pub async fn main() -> anyhow::Result<()> {
    let args = Cli::parse();

    let _guard = args.logging.init()?;

    // TODO: de-duplicate this!
    match args.netconf {
        None | Some(NetconfOpts::Local) => {
            let updater = Updater::new(Local, args.irrd, args.junos);

            match args.frequency {
                Frequency::OneShot => updater.run().await,
                Frequency::Daemon(frequency) => updater.init_loop(frequency).start().await,
            }
        }
        Some(NetconfOpts::Remote(opts)) => {
            let updater = Updater::new(Remote::new(opts), args.irrd, args.junos);

            match args.frequency {
                Frequency::OneShot => updater.run().await,
                Frequency::Daemon(frequency) => updater.init_loop(frequency).start().await,
            }
        }
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

    #[command(flatten, next_help_heading = "IRR connection options")]
    irrd: IrrdOpts,

    #[command(flatten, next_help_heading = "Logging options")]
    logging: LoggingOpts,

    #[command(subcommand)]
    netconf: Option<NetconfOpts>,
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

impl Display for Frequency {
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

#[derive(Debug, Subcommand)]
#[command(
    subcommand_help_heading = "NETCONF target [default: local]",
    subcommand_value_name = "TARGET"
)]
pub(super) enum NetconfOpts {
    Local,
    Remote(NetconfTlsOpts),
}

#[derive(Debug, Args)]
pub(super) struct NetconfTlsOpts {
    /// NETCONF server hostname or IP address.
    #[arg(long = "netconf-host", id = "netconf-host", value_name = "HOST")]
    #[cfg_attr(target_platform = "junos-freebsd", arg(default_value = "localhost"))]
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
    #[arg(long, default_value_os_t = Self::default_pki_path("ca.crt"))]
    ca_cert_path: PathBuf,

    /// NETCONF TLS transport client certificate path.
    #[arg(long, default_value_os_t = Self::default_pki_path("client.crt"))]
    client_cert_path: PathBuf,

    /// NETCONF TLS transport client private key path.
    #[arg(long, default_value_os_t = Self::default_pki_path("client.key"))]
    client_key_path: PathBuf,

    /// Override the domain name against which the NETCONF server's TLS certificate is verified.
    #[arg(long, value_parser = parse_server_name)]
    #[cfg_attr(target_platform = "junos-freebsd", arg(default_value_os = hostname::get()))]
    tls_server_name: Option<ServerName<'static>>,
}

#[cfg(target_platform = "junos-freebsd")]
mod hostname {
    use once_cell::sync::Lazy;
    use std::ffi::{OsStr, OsString};

    static HOSTNAME: Lazy<OsString> = Lazy::new(gethostname::gethostname);

    pub(super) fn get() -> &'static OsStr {
        HOSTNAME.as_os_str()
    }
}

fn parse_server_name(name: &str) -> anyhow::Result<ServerName<'static>> {
    let parsed = ServerName::try_from(name).context("failed to parse server name")?;
    Ok(parsed.to_owned())
}

impl NetconfTlsOpts {
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

    fn default_pki_path<P: AsRef<Path>>(file_name: P) -> PathBuf {
        let pki_dir = if cfg!(target_platform = "junos-freebsd") {
            // TODO:
            // Is this the right place to keep these?
            // Is there a way of using the Junos PKI infrastructure?
            Path::new("/var/db/bgpfu")
        } else {
            Path::new("./certs")
        };
        pki_dir.join(file_name)
    }
}

#[derive(Debug, Args)]
struct LoggingOpts {
    /// Logging output destination
    #[arg(short = 'l', long, default_value_t)]
    logging_dest: LoggingDest<PathBuf>,

    /// Size at which log file is rotated
    #[arg(long, default_value_t = FileSize(ByteUnit::Megabyte(10)))]
    log_file_size: FileSize,

    #[command(flatten)]
    verbosity: Verbosity<InfoLevel>,
}

impl LoggingOpts {
    fn init(self) -> anyhow::Result<WorkerGuard> {
        let level = self.verbosity.log_level_filter().as_trace();
        let filter = EnvFilter::builder()
            .with_default_directive(level.into())
            .from_env_lossy();
        let builder = tracing_subscriber::fmt()
            .compact()
            .with_env_filter(filter)
            .with_ansi(self.logging_dest.emit_colours());
        let (non_blocking, guard) = self.logging_dest.open(self.log_file_size)?;
        builder
            .with_writer(non_blocking)
            .try_init()
            .map_err(|err| anyhow!(err))?;
        Ok(guard)
    }
}

#[derive(Debug, Clone)]
enum LoggingDest<T> {
    StdErr,
    File(T),
}

impl<T> LoggingDest<T> {
    const fn emit_colours(&self) -> bool {
        match self {
            Self::StdErr => true,
            Self::File(_) => false,
        }
    }
}

impl Default for LoggingDest<PathBuf> {
    fn default() -> Self {
        if cfg!(target_platform = "junos-freebsd") {
            Self::File("/var/log/bgpfu-junos-agent.log".into())
        } else {
            Self::StdErr
        }
    }
}

impl Display for LoggingDest<PathBuf> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::StdErr => write!(f, "STDERR"),
            Self::File(path) => path.to_string_lossy().fmt(f),
        }
    }
}

impl FromStr for LoggingDest<PathBuf> {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        if s == "STDERR" {
            Ok(Self::StdErr)
        } else {
            Ok(Self::File(s.into()))
        }
    }
}

impl LoggingDest<PathBuf> {
    fn open(self, max_size: FileSize) -> anyhow::Result<(NonBlocking, WorkerGuard)> {
        let builder = NonBlockingBuilder::default().lossy(false);
        match self {
            Self::StdErr => Ok(builder.finish(std::io::stderr())),
            Self::File(path) => {
                let writer = BasicRollingFileAppender::new(
                    path,
                    RollingConditionBasic::new().max_size(max_size.to_u64()),
                    9,
                )
                .context("failed to open log file '{path.display()}'")?;
                Ok(builder.finish(writer))
            }
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
struct FileSize(ByteUnit);

impl FileSize {
    fn to_u64(self) -> u64 {
        self.0.into()
    }
}

impl Display for FileSize {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
    }
}

impl FromStr for FileSize {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        s.parse::<ByteUnit>().map_err(|err| anyhow!(err)).map(Self)
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
