use clap::Parser;
use log::LevelFilter;

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
pub struct Args {
    /// Save the current layout (prints to stdout). If not specified, then the layout in ~/.layout.yaml will be loaded.
    #[arg(short, long)]
    pub save: bool,

    /// Logging level. Default: Info. Valid values: Off, Error, Warn, Info, Debug, Trace,
    #[arg(short, long, default_value = "Info")]
    pub log_level: LevelFilter,
}
