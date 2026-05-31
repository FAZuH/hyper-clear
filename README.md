# hyper-clear

CLI tool to control window transparency and blur on Hyprland.

## Usage

```
Usage: hyper-clear [OPTIONS] <COMMAND>

Commands:
  blur-toggle       Toggle blur effect on/off [aliases: bt]
  opacity-toggle    Toggle opacity override on/off [aliases: ot, toggle]
  opacity-increase  Increase window opacity by 10% [aliases: oi, inc]
  opacity-decrease  Decrease window opacity by 10% [aliases: od, dec]
  opacity-set       Set window opacity to a specific percentage [aliases: os, set]
  status            Show current window state [aliases: st, info, show]
  help              Print this message or the help of the given subcommand(s)

Options:
  -v, --verbose...     Verbosity level (-v, -vv, -vvv, -vvvv)
      --color <COLOR>  Color output mode [auto|always|never] [default: auto]
  -h, --help           Print help
  -V, --version        Print version
```
