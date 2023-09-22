use error::HpgError;
use lazy_static::lazy_static;
use output::StructuredWriter;
use std::collections::HashMap;
use std::fs::File;
use structopt::StructOpt;
use task::LuaState;
use task::Variables;
use tasks::TaskRef;

pub(crate) mod actions;
mod error;
mod hash;
mod lua;
pub(crate) mod modules;
mod output;
mod task;
mod tasks;

pub type Result<T, E = HpgError> = core::result::Result<T, E>;
use std::io::prelude::*;
use std::io::BufReader;

use crate::output::Target;

lazy_static! {
    pub static ref WRITER: StructuredWriter = StructuredWriter::new(Target::Stdout);
}

fn load_file(fname: &str) -> Result<String, HpgError> {
    let f = File::open(fname)?;
    let mut reader = BufReader::new(f);
    let mut s = String::new();
    reader.read_to_string(&mut s)?;
    Ok(s)
}

fn parse_variable(s: &str) -> Result<(String, String)> {
    let (k, v) = s
        .split_once("=")
        .ok_or_else(|| HpgError::ParseError("Invalid Variable: Missing '='".to_string()))?;
    Ok((k.to_string(), v.to_string()))
}

#[derive(Debug, StructOpt)]
#[structopt(name = "hpg", about = "config management tool")]
struct Opt {
    #[structopt(
        short,
        long,
        name = "CONFIG",
        default_value = "hpg.lua",
        help = "Path to hpg config file"
    )]
    config: String,
    #[structopt(
        short = "D",
        long = "default-targets",
        name = "default-targets",
        help = "Run default targets in config"
    )]
    run_defaults: bool,
    #[structopt(
        short = "v",
        long = "var",
        name = "KEY=VALUE",
        help = "Key-value pairs to add as variables",
        parse(try_from_str = parse_variable),
        conflicts_with("VARS-FILE")
    )]
    variables: Vec<(String, String)>,
    #[structopt(
        long = "vars",
        name = "VARS-FILE",
        help = "Path to JSON variables file"
    )]
    var_file: Option<String>,
    #[structopt(
        long = "lsp-defs",
        help = "Output LSP definitions for HPG to .meta/hpgdefs.lua.  Compatible with EmmyLua and lua-language-server."
    )]
    lsp_defs: bool,
    #[structopt(
        long = "raw-lsp-defs",
        help = "Output LSP definitions for HPG to stdout.  Compatible with EmmyLua and lua-language-server."
    )]
    raw_lsp_defs: bool,
    #[structopt(short, long, help = "Show planned execution but do not execute")]
    show: bool,
    #[structopt(name = "TARGETS", help = "Task names to run")]
    targets: Vec<String>,
}

fn lsp_defs() -> &'static str {
    include_str!("hpgdefs.lua")
}

fn run_hpg() -> Result<()> {
    let opt = Opt::from_args();
    if opt.lsp_defs {
        let path = std::path::PathBuf::from("./.meta");
        std::fs::create_dir_all(&path)?;
        let mut f = std::fs::File::options()
            .create(true)
            .write(true)
            .truncate(true)
            .open(path.join("hpgdefs.lua"))?;
        f.write_all(&lsp_defs().as_bytes())?;
        return Ok(());
    }
    if opt.raw_lsp_defs {
        println!("{}", lsp_defs());
        return Ok(());
    }
    let code = load_file(&opt.config)?;
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

    let v = if let Some(f) = opt.var_file {
        let s = load_file(&f)?;
        let json = serde_json::from_str(&s)
            .map_err(|e| HpgError::ParseError(format!("Invalid vars file: {}", e)))?;
        Variables::from_json(json)
    } else {
        let vars: HashMap<String, String> = opt.variables.into_iter().collect();
        let json = serde_json::to_value(&vars).unwrap();
        Variables::from_json(json)
    };
    let lua = lua.eval(&code, v)?;
    let requested_tasks: Vec<&str> = opt.targets.iter().map(|t| t.as_str()).collect();
    lua.execute(&requested_tasks, opt.run_defaults, opt.show)?;

    Ok(())
}

fn main() -> Result<()> {
    if let Err(e) = run_hpg() {
        match e {
            HpgError::TaskError(t) => match t {
                error::TaskError::CycleError(c) => eprintln!("Cycle detected in task {}", c),
                error::TaskError::UnknownTask(t) => eprintln!("Unknown task: {}", t),
                error::TaskError::LuaError(l) => eprintln!("Lua Error: {}", l),
                error::TaskError::IoError(i) => eprintln!("IO Error: {}", i),
                error::TaskError::ActionError(a) => eprintln!("Error in action: {}", a),
                error::TaskError::SkippedTask => eprintln!("Skipped Task."),
                error::TaskError::TemplateError(t) => eprintln!("Error in template: {}", t),
                error::TaskError::DbusError(d) => eprintln!("Dbus error: {}", d),
            },
            HpgError::FileError(f) => eprintln!("Error loading file: {}", f),
            HpgError::ParseError(p) => eprintln!("Failed parsing: {}", p),
        }
    }
    Ok(())
}
