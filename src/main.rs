use error::HpgError;
use lazy_static::lazy_static;
use lua::LuaState;
use output::StructuredWriter;
use std::fs::File;
use structopt::StructOpt;
use tasks::TaskRef;

mod actions;
mod error;
mod lua;
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
    #[structopt(short, long, name = "CONFIG", default_value = "hpg.lua")]
    config: String,
    #[structopt(name = "TARGETS")]
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
    lua.register_fn(actions::copy)?;
    let lua = lua.eval(&code)?;
    lua.execute(&task_refs)?;

    Ok(())
}
