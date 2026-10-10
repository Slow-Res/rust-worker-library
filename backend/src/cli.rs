//! Command-line interface (clap). Adding a feature subcommand is two local
//! edits: a variant in [`Command`] and a match arm in `main.rs`.

use clap::{Parser, Subcommand};

#[derive(Debug, Parser)]
#[command(
    name = "library-backend",
    version,
    about = "Library management backend"
)]
pub struct Cli {
    /// Defaults to `serve` when omitted.
    #[command(subcommand)]
    command: Option<Command>,
}

#[derive(Debug, Clone, PartialEq, Eq, Subcommand)]
pub enum Command {
    /// Run the HTTP server.
    Serve,
}

impl Cli {
    /// The chosen subcommand, or `serve` if none was given.
    pub fn command(self) -> Command {
        self.command.unwrap_or(Command::Serve)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn no_subcommand_means_serve() {
        let cli = Cli::parse_from(["library-backend"]);
        assert_eq!(cli.command(), Command::Serve);
    }

    #[test]
    fn explicit_serve() {
        let cli = Cli::parse_from(["library-backend", "serve"]);
        assert_eq!(cli.command(), Command::Serve);
    }
}
