use clap::{AppSettings, Clap};

use crate::cmd::Cmd;

/// An IRR query and filter generation toolset.
#[derive(Clap, Debug)]
#[clap(
    rename_all = "kebab_case",
    after_help = "See '--help' for detailed usage information.\n",
    global_setting = AppSettings::ColoredHelp,
    global_setting = AppSettings::DisableHelpSubcommand,
)]
pub struct Args {
    /// IRRd server address and port.
    #[clap(short, long, global = true, default_value = "whois.radb.net:43")]
    addr: String,

    /// Increase logging verbosity.
    ///
    /// This flag may be repeated to further increase logging detail.
    /// By default, logs are emitted at the WARNING level.
    #[clap(
        short = 'v',
        group = "verbosity",
        parse(from_occurrences),
        global = true
    )]
    verbosity_pos: usize,

    /// Decrease logging verbosity.
    ///
    /// This flag may be repeated to further decrease logging detail.
    /// By default, logs are emitted at the WARNING level.
    #[clap(
        short = 'q',
        group = "verbosity",
        parse(from_occurrences),
        global = true
    )]
    verbosity_neg: usize,

    /// Logging timestamp format.
    #[clap(
        long,
        possible_values = &["sec", "ms", "us", "ns", "off"],
        default_value = "off",
        global = true,
    )]
    log_timestamp: stderrlog::Timestamp,

    #[clap(subcommand)]
    command: Cmd,
}

impl Args {
    /// Construct socket address for IRR client connection.
    // pub fn addr(&self) -> String {
    //     format!("{}:{}", self.host, self.port)
    // }

    /// Get log timestamping option.
    pub fn log_timestamp(&self) -> stderrlog::Timestamp {
        self.log_timestamp
    }

    /// Calculate logging verbosity.
    pub fn verbosity(&self) -> usize {
        1 + self.verbosity_pos - self.verbosity_neg
    }

    /// Get command.
    pub fn command(&self) -> &Cmd {
        &self.command
    }
}
