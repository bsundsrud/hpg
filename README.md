# HPG

Lua-based configuration management.

## Architecture

Tasks are defined in lua along with any dependencies, and requested tasks and
their dependencies are executed, in appropriate order. Default targets can be
set and executed with the `-D` flag, allowing the entirety of the config and
its execution to be committed to version control.

See the [reference](docs/reference.md) for the Lua API and guide.

## Usage

### Top-level command

```
Usage: hpg [OPTIONS] [COMMAND]

Commands:
  local  Run HPG Locally
  ssh    Run HPG over SSH
  help   Print this message or the help of the given subcommand(s)

Options:
      --lsp-defs      Output LSP definitions for HPG to .meta/hpgdefs.lua.  Compatible with EmmyLua and lua-language-server.
      --raw-lsp-defs  Output LSP definitions for HPG to stdout.  Compatible with EmmyLua and lua-language-server.
      --debug         Show debug output
  -h, --help          Print help
  -V, --version       Print version
```

### Run locally

```
Usage: hpg local [OPTIONS] [TARGETS]...

Arguments:
  [TARGETS]...  Task names to run

Options:
  -c, --config <CONFIG>            Path to hpg config file, relative to project-dir [default: hpg.lua]
  -p, --project-dir <PROJECT_DIR>  Path to project root. Default is the current directory [default: .]
  -D, --default-targets            Run default targets in config
  -v, --var <KEY=VALUE>            Key-value pairs to add as variables
      --vars <VARS-FILE>           Path to JSON variables file
  -s, --show                       Show planned execution but do not execute
  -l, --list                       Show available targets
  -h, --help                       Print help
  -V, --version                    Print version
```

### Run Remotely via SSH
(currently only supports private key auth)

```
Usage: hpg ssh [OPTIONS] <[USER@]HOST[:PORT]> [TARGETS]...

Arguments:
  <[USER@]HOST[:PORT]>  Remote host address
  [TARGETS]...          Task names to run

Options:
  -i, --inventory <INVENTORY>      Path to inventory file
  -c, --config <CONFIG>            Path to hpg config file, relative to project-dir [default: hpg.lua]
  -p, --project-dir <PROJECT_DIR>  Path to project root. Default is the current directory [default: .]
  -D, --default-targets            Run default targets in config
  -v, --var <KEY=VALUE>            Key-value pairs to add as variables
      --vars <VARS-FILE>           Path to JSON variables file
  -s, --show                       Show planned execution but do not execute
  -l, --list                       Show available targets
  -h, --help                       Print help
  -V, --version                    Print version
```