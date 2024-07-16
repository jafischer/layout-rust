use clap::{Parser, Subcommand};
use log::LevelFilter;

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
/// A tool for restoring your carefully-arranged window layout on your MacBook.
pub struct Args {
    /// Logging level. Valid values: off, error, warn, info, debug, trace.
    #[arg(short, long, default_value = "info", global = true)]
    pub log_level: LevelFilter,

    /// The path to the layout file.
    #[arg(short, long, default_value = "~/.layout.yaml")]
    pub path: String,

    #[command(subcommand)]
    command: Option<Command>,
}

#[derive(Subcommand, Clone, Debug)]
pub enum Command {
    /// Restore the layout. This is the default action.
    Restore,
    /// "Save" (print to stdout) the current window layout.
    Save,
}

impl Args {
    pub fn command(&self) -> Command {
        self.command.clone().unwrap_or(Command::Restore)
    }
}
