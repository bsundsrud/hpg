# HPG

Lua-based configuration management.

## Architecture

Tasks are defined in lua along with any dependencies, and requested tasks and
their dependencies are executed, in appropriate order. Default targets can be
set and executed with the `-D` flag, allowing the entirety of the config and
its execution to be committed to version control.

See the [reference](docs/reference.md) for the Lua API and guide.

## Usage

```
hpg 0.1.0
config management tool

USAGE:
    hpg [FLAGS] [OPTIONS] [TARGETS]...

FLAGS:
    -D, --default-targets    Run default targets in config
    -h, --help               Prints help information
    -V, --version            Prints version information

OPTIONS:
    -c, --config <CONFIG>    Path to hpg config file [default: hpg.lua]

ARGS:
    <TARGETS>...    Task names to run
```
