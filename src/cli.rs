use clap::Parser;
use clap::Subcommand;
use fazuh_common::log::cli::ColorMode;
use fazuh_common::types::Percent;

#[derive(Parser)]
#[command(version, about, long_about = None)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Command,

    /// Verbosity level (-v, -vv, -vvv, -vvvv)
    #[arg(short, long, global = true, action = clap::ArgAction::Count)]
    pub verbose: u8,

    /// Color output mode [auto|always|never]
    #[arg(
        long,
        global = true,
        default_value = "auto",
        value_enum,
        hide_possible_values = true
    )]
    pub color: ColorMode,
}

#[derive(Subcommand)]
pub enum Command {
    /// Toggle blur effect on/off
    #[command(visible_alias = "bt")]
    BlurToggle,
    /// Toggle opacity override on/off
    #[command(visible_aliases = ["ot", "toggle"])]
    OpacityToggle,
    /// Increase window opacity by 10%
    #[command(visible_aliases = ["oi", "inc"])]
    OpacityIncrease,
    /// Decrease window opacity by 10%
    #[command(visible_aliases = ["od", "dec"])]
    OpacityDecrease,
    /// Set window opacity to a specific percentage
    #[command(visible_aliases = ["os", "set"])]
    OpacitySet { value: Percent },
    /// Show current window state
    #[command(visible_aliases = ["st", "info", "show"])]
    Status,
}
