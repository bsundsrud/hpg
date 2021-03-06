use error::HpgError;
use lazy_static::lazy_static;
use lua::LuaState;
use output::StructuredWriter;
use std::fs::File;
use structopt::StructOpt;
use tasks::TaskRef;

pub(crate) mod actions;
mod error;
mod hash;
mod lua;
pub(crate) mod modules;
mod output;
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

fn main() -> Result<()> {
    let opt = Opt::from_args();
    if opt.lsp_defs {
        let path = std::path::PathBuf::from("./.meta");
        std::fs::create_dir_all(&path)?;
        let mut f = std::fs::File::options()
            .create(true)
            .write(true)
            .open(path.join("hpgdefs.lua"))?;
        f.write_all(&lsp_defs().as_bytes())?;
        return Ok(());
    }
    if opt.raw_lsp_defs {
        println!("{}", lsp_defs());
        return Ok(());
    }
    let code = load_file(&opt.config)?;
    let task_refs: Vec<TaskRef> = opt.targets.into_iter().map(TaskRef::new).collect();
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
    lua.register_fn(modules::pkg)?;
    lua.register_fn(modules::machine)?;
    lua.register_fn(modules::url)?;
    lua.register_fn(modules::archive)?;
    lua.register_fn(modules::installer)?;
    lua.register_fn(modules::systemd_service)?;
    let lua = lua.eval(&code)?;
    lua.execute(&task_refs, opt.run_defaults, opt.show)?;

    Ok(())
}
