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
    #[structopt(name = "TARGETS", help = "Task names to run")]
    targets: Vec<String>,
}

fn main() -> Result<()> {
    let opt = Opt::from_args();
    let code = load_file(&opt.config)?;
    let task_refs: Vec<TaskRef> = opt.targets.into_iter().map(TaskRef::new).collect();
    let lua = LuaState::new()?;
    lua.register_fn(actions::echo)?;
    lua.register_fn(actions::fail)?;
    lua.register_fn(actions::exec)?;
    lua.register_fn(actions::shell)?;
    lua.register_fn(actions::hash_text)?;
    lua.register_fn(actions::package)?;
    lua.register_fn(actions::cancel)?;
    lua.register_fn(actions::success)?;
    lua.register_fn(actions::user)?;
    lua.register_fn(actions::user_exists_action)?;
    lua.register_fn(actions::group)?;
    lua.register_fn(actions::group_exists_action)?;
    lua.register_fn(actions::from_json)?;
    lua.register_fn(modules::file)?;
    lua.register_fn(modules::dir)?;

    let lua = lua.eval(&code)?;
    lua.execute(&task_refs, opt.run_defaults)?;

    Ok(())
}
