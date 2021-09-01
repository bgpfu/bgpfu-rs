use std::io::Write;

use anyhow::{anyhow, Result};
use clap::{Clap, IntoApp};
use clap_generate::{generate, generators::*, Shell};

use crate::cli::Args;

use super::Dispatch;

#[derive(Clap, Debug)]
pub struct Completion {
    /// shell to generate completion script for.
    #[clap(possible_values = &Shell::variants())]
    shell: Shell,
}

impl<W: Write> Dispatch<W> for Completion {
    fn dispatch(&self, writer: &mut W) -> Result<()> {
        match self.shell {
            Shell::Bash => write_completions::<Bash, _>(writer),
            Shell::Elvish => write_completions::<Elvish, _>(writer),
            Shell::Fish => write_completions::<Fish, _>(writer),
            Shell::PowerShell => write_completions::<PowerShell, _>(writer),
            Shell::Zsh => write_completions::<Zsh, _>(writer),
            shell => Err(anyhow!("completion not available for {}", shell)),
        }
    }
}

fn write_completions<G, W>(w: &mut W) -> Result<()>
where
    G: Generator,
    W: Write,
{
    let mut app = Args::into_app();
    let name = app.get_name().to_string();
    generate::<G, _>(&mut app, name, w);
    Ok(())
}
