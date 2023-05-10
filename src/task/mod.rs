use anyhow::Result;
use mlua::{self, Function, Lua, Table, UserData, Value};
use std::{fs::read_to_string, sync::atomic::AtomicUsize};

#[derive(Debug, Clone, Copy)]
pub struct TaskSigil {
    id: usize,
}

#[derive(Debug, Clone)]
pub struct ResolvedTask {
    name: String,
    id: usize,
}

impl UserData for TaskSigil {}

fn find_tasks(ctx: &Lua) -> Result<Vec<ResolvedTask>> {
    let globals = ctx.globals();
    let mut tasks = Vec::new();
    for pair in globals.pairs() {
        let (name, val): (String, Value) = pair?;
        match val {
            Value::UserData(ud) => {
                if ud.is::<TaskSigil>() {
                    let ts: TaskSigil = *ud.borrow::<TaskSigil>()?;
                    println!("TaskSigil: {:?}", ts);
                    tasks.push(ResolvedTask { name, id: ts.id });
                }
            }
            _ => continue,
        }
    }
    Ok(tasks)
}

fn run() -> Result<()> {
    let code = read_to_string("test.lua")?;
    let lua = Lua::new();
    let id = AtomicUsize::new(1);
    let task_table = lua.create_table()?;
    lua.set_named_registry_value("tasks", task_table)?;

    let f = lua.create_function(move |ctx, f: Function| {
        let task_table: Table = ctx.named_registry_value("tasks")?;
        let i = id.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        task_table.set(i, f)?;
        ctx.set_named_registry_value("tasks", task_table)?;
        let sigil = TaskSigil { id: i };
        Ok(sigil)
    })?;

    lua.globals().set("task", f)?;
    lua.load(&code).exec()?;
    let tasks = find_tasks(&lua)?;

    let task_table: Table = lua.named_registry_value("tasks")?;
    for t in tasks {
        println!("{}: {}", t.id, t.name);
        let f = task_table.get::<usize, Function>(t.id)?;
        let _ = f.call(())?;
    }

    for pair in task_table.pairs() {
        let (id, t): (usize, Function) = pair?;
        println!("All Tasks: {}", id);
    }

    Ok(())
}
