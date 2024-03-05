use clap::Parser;
use log::LevelFilter;

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
/// A tool for restoring your carefully-arranged window layout on your MacBook.
pub struct Args {
    /// Save the current layout (prints to stdout). If --save is not specified,
    /// then the layout in ~/.layout.yaml will be restored.
    #[arg(short, long)]
    pub save: bool,

    /// Logging level. Default: Info. Valid values: Off, Error, Warn, Info, Debug, Trace,
    #[arg(short, long, default_value = "info")]
    pub log_level: LevelFilter,
}
