use clap::Parser;
use color_eyre::Result;
use fazuh_common::log::LogConfigBuilder;
use fazuh_common::log::cli::get_ansi;

use crate::cmd::OpacityOverride;

pub mod cli;
pub mod cmd;

fn main() -> Result<()> {
    color_eyre::install()?;

    let cli = cli::Cli::parse();

    let conf = LogConfigBuilder::default()
        .verbosity(cli.verbose)
        .ansi(get_ansi(cli.color))
        .build()?;
    let _ = fazuh_common::log::install_logging(conf);

    cmd::ensure_deps()?;

    use cli::Command::*;
    match cli.command {
        BlurToggle => cmd::toggle_blur()?,
        OpacityToggle => cmd::toggle_opacity()?,
        OpacityIncrease => cmd::increase_opacity()?,
        OpacityDecrease => cmd::decrease_opacity()?,
        OpacitySet { value } => cmd::pub_set_opacity(OpacityOverride::new_equal(value))?,
        Status => println!("{}", cmd::pub_get_opacity()?),
    }

    Ok(())
}
