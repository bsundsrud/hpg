use clap::builder::TypedValueParser;
use clap::Args;
use clap::CommandFactory;
use clap::Parser;
use clap::Subcommand;
use console::style;
use error::HpgError;
use error::HpgRemoteError;

use remote::config::InventoryConfig;
use remote::ssh::HostInfo;

use std::collections::HashMap;
use std::fs::File;

use std::path::PathBuf;

use task::LuaState;
use task::Variables;

pub(crate) mod actions;
mod error;
mod hash;
mod macros;
pub(crate) mod modules;

mod remote;
mod task;
mod tracker;

pub type Result<T, E = HpgError> = core::result::Result<T, E>;
use std::io::prelude::*;
use std::io::BufReader;

pub fn load_file(fname: &str) -> Result<String, HpgError> {
    let f = File::open(fname)?;
    let mut reader = BufReader::new(f);
    let mut s = String::new();
    reader.read_to_string(&mut s)?;
    Ok(s)
}

fn parse_variable(s: &str) -> Result<(String, String)> {
    let (k, v) = s
        .split_once('=')
        .ok_or_else(|| HpgError::Parse("Invalid Variable: Missing '='".to_string()))?;
    Ok((k.to_string(), v.to_string()))
}

fn try_parse_host(host_str: &str) -> Result<HostInfo> {
    let (user, rest) = if let Some((u, rest)) = host_str.split_once("@") {
        (Some(u.to_string()), rest)
    } else {
        (None, host_str)
    };

    let (hostname, port) = if let Some((h, p)) = rest.split_once(":") {
        let port = Some(p.parse::<u16>().map_err(|_e| HpgRemoteError::ParseHost {
            orig: host_str.to_string(),
            reason: "Could not parse port".into(),
        })?);
        (h.into(), port)
    } else {
        (rest.into(), None)
    };

    Ok(HostInfo {
        hostname,
        port,
        user,
    })
}

#[derive(Debug, Clone)]
struct ProjectDirParser {}

impl ProjectDirParser {
    fn new() -> Self {
        Self {}
    }
}
impl TypedValueParser for ProjectDirParser {
    type Value = PathBuf;

    fn parse_ref(
        &self,
        _cmd: &clap::Command,
        _arg: Option<&clap::Arg>,
        value: &std::ffi::OsStr,
    ) -> Result<Self::Value, clap::Error> {
        let s = value.to_string_lossy().to_string();
        let p = PathBuf::from(s);
        Ok(p.canonicalize()?)
    }
}

#[derive(Debug, Parser)]
#[command(about, version)]
#[command(propagate_version = true)]
struct Opt {
    #[command(flatten)]
    globals: GlobalOpt,
    #[command(subcommand)]
    cmd: Option<RemoteCommands>,
}

#[derive(Debug, Subcommand)]
enum RemoteCommands {
    #[command(about = "Run HPG Locally")]
    Local {
        #[command(flatten)]
        hpg_opts: HpgOpt,
    },
    #[command(about = "Run HPG over SSH")]
    Ssh {
        #[arg(short, long, name = "INVENTORY", help = "Path to inventory file")]
        inventory: Option<String>,
        #[arg(
            name = "[USER@]HOST[:PORT]",
            help = "Remote host address",
            value_parser(try_parse_host)
        )]
        host: HostInfo,
        #[command(flatten)]
        hpg_opts: HpgOpt,
    },
    #[command(hide(true))]
    Server {
        #[arg(name = "ROOT-DIR", help = "Base dir for HPG sync")]
        root_dir: String,
    },
}

#[derive(Debug, Args)]
struct GlobalOpt {
    #[arg(
        long = "lsp-defs",
        help = "Output LSP definitions for HPG to .meta/hpgdefs.lua.  Compatible with EmmyLua and lua-language-server."
    )]
    lsp_defs: bool,
    #[arg(
        long = "raw-lsp-defs",
        help = "Output LSP definitions for HPG to stdout.  Compatible with EmmyLua and lua-language-server."
    )]
    raw_lsp_defs: bool,
    #[arg(long, help = "Show debug output")]
    debug: bool,
}

#[derive(Debug, Parser)]
pub struct HpgOpt {
    #[arg(
        short,
        long,
        name = "CONFIG",
        default_value = "hpg.lua",
        help = "Path to hpg config file"
    )]
    config: String,
    #[arg(
        short,
        long,
        help = "Path to project root. Default is the current directory",
        required = false,
        default_value = ".",
        value_parser(ProjectDirParser::new())
    )]
    project_dir: PathBuf,
    #[arg(
        short = 'D',
        long = "default-targets",
        name = "default-targets",
        help = "Run default targets in config"
    )]
    run_defaults: bool,
    #[arg(
        short = 'v',
        long = "var",
        name = "KEY=VALUE",
        help = "Key-value pairs to add as variables",
        value_parser(parse_variable)
    )]
    variables: Vec<(String, String)>,
    #[arg(
        long = "vars",
        name = "VARS-FILE",
        help = "Path to JSON variables file"
    )]
    var_file: Vec<String>,
    #[arg(short, long, help = "Show planned execution but do not execute")]
    show: bool,
    #[arg(short, long, help = "Show available targets")]
    list: bool,
    #[arg(name = "TARGETS", help = "Task names to run")]
    targets: Vec<String>,
}

fn lsp_defs() -> &'static str {
    include_str!("hpgdefs.lua")
}

fn parse_variables(opt: &HpgOpt) -> Result<Variables> {
    let vars: HashMap<String, String> = opt.variables.clone().into_iter().collect();
    let mut v = Variables::from_map(&vars)?;

    for f in opt.var_file.iter() {
        let file_vars = Variables::from_file(&f)?;
        v = file_vars.merge(v)?;
    }
    Ok(v)
}

fn try_inventory_files(paths: &[&str]) -> Result<InventoryConfig> {
    for f in paths {
        let p = PathBuf::from(f);
        if p.exists() {
            return Ok(InventoryConfig::load(p)?);
        }
    }
    Ok(InventoryConfig::default())
}

fn run_hpg_local(opt: HpgOpt, lua: LuaState) -> Result<()> {
    std::env::set_current_dir(&opt.project_dir)?;
    let vars = parse_variables(&opt)?;
    let code = load_file(&opt.config)?;

    let lua = lua.eval(&code, vars)?;
    if opt.list {
        output!("{}", style("Available Tasks").cyan());
        for (name, task) in lua.available_targets() {
            indent_output!(1, "{}: {}", style(name).green(), task.description());
        }
        return Ok(());
    }
    let requested_tasks: Vec<&str> = opt.targets.iter().map(|t| t.as_str()).collect();
    lua.execute(&requested_tasks, opt.run_defaults, opt.show)?;

    Ok(())
}

fn run_hpg() -> Result<()> {
    let opt = Opt::parse();
    if opt.globals.lsp_defs {
        let path = std::path::PathBuf::from("./.meta");
        std::fs::create_dir_all(&path)?;
        let mut f = std::fs::File::options()
            .create(true)
            .write(true)
            .truncate(true)
            .open(path.join("hpgdefs.lua"))?;
        f.write_all(lsp_defs().as_bytes())?;
        return Ok(());
    }
    if opt.globals.raw_lsp_defs {
        println!("{}", lsp_defs());
        return Ok(());
    }

    let lua = LuaState::new()?;
    lua.register_fn(actions::echo)?;
    lua.register_fn(actions::fail)?;
    lua.register_fn(actions::exec)?;
    lua.register_fn(actions::shell)?;
    lua.register_fn(actions::hash_text)?;
    lua.register_fn(actions::cancel)?;
    lua.register_fn(actions::success)?;
    lua.register_fn(actions::user)?;
    lua.register_fn(actions::user_exists_action)?;
    lua.register_fn(actions::group)?;
    lua.register_fn(actions::group_exists_action)?;
    lua.register_fn(actions::from_json)?;
    lua.register_fn(modules::file)?;
    lua.register_fn(modules::dir)?;
    lua.register_fn(modules::homedir)?;
    lua.register_fn(modules::pkg)?;
    lua.register_fn(modules::machine)?;
    lua.register_fn(modules::url)?;
    lua.register_fn(modules::archive)?;
    lua.register_fn(modules::installer)?;
    lua.register_fn(modules::systemd_service)?;
    lua.register_fn(modules::user)?;

    match opt.cmd {
        Some(RemoteCommands::Local { hpg_opts }) => {
            let handle = tracker::init(opt.globals.debug)?;

            let res = run_hpg_local(hpg_opts, lua);
            handle.finish();
            res
        }
        Some(RemoteCommands::Ssh {
            host,
            hpg_opts,
            inventory,
        }) => {
            let handle = tracker::init(opt.globals.debug)?;
            let inventory = if let Some(p) = inventory {
                try_inventory_files(&[&p])?
            } else {
                try_inventory_files(&[
                    "inventory.yaml",
                    "inventory.yml",
                    "inventory.hjson",
                    "inventory.json",
                ])?
            };
            let vars = parse_variables(&hpg_opts)?;
            remote::ssh::run_hpg_ssh(host, hpg_opts, vars, inventory)?;
            handle.finish();
            Ok(())
        }
        Some(RemoteCommands::Server { root_dir }) => {
            let handle = tracker::init(opt.globals.debug)?;
            remote::server::run_socket_server(root_dir, lua, &PathBuf::from("/tmp/hpg.socket"))?;
            handle.finish();
            Ok(())
        }
        None => {
            Opt::command().print_long_help()?;
            Ok(())
        }
    }
}

fn main() -> Result<()> {
    if let Err(e) = run_hpg() {
        match e {
            HpgError::Task(t) => match t {
                error::TaskError::Cycle(c) => eprintln!("Cycle detected in task {}", c),
                error::TaskError::UnknownTask(t) => eprintln!("Unknown task: {}", t),
                error::TaskError::Lua(l) => eprintln!("Lua Error: {}", l),
                error::TaskError::Io(i) => eprintln!("IO Error: {}", i),
                error::TaskError::Action(a) => eprintln!("Error in action: {}", a),
                error::TaskError::SkippedTask => {}
                error::TaskError::Template(t) => eprintln!("Error in template: {}", t),
                error::TaskError::Dbus(d) => eprintln!("Dbus error: {}", d),
            },
            HpgError::Remote(r) => {
                eprintln!("Remote Error: {}", r);
            }
            HpgError::File(f) => eprintln!("Error loading file: {}", f),
            HpgError::Parse(p) => eprintln!("Failed parsing: {}", p),
            HpgError::Serde(e) => eprintln!("Failed to parse json: {}", e),
            HpgError::Other(e) => eprintln!("{}", e),
        }
    }
    Ok(())
}
