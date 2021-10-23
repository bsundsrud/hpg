use std::sync::{Arc, Mutex};

use crate::{
    error::TaskError,
    tasks::{TaskDefinition, TaskGraphState, TaskRef},
    Result,
};
use rlua::{Function, Lua, Table};

pub struct LuaState {
    lua: Lua,
    tasks: Arc<Mutex<Vec<TaskDefinition>>>,
}

impl LuaState {
    pub fn new() -> Result<Self> {
        let lua = Self {
            lua: Lua::new(),
            tasks: Arc::new(Mutex::new(Vec::new())),
        };
        lua.task_defines()?;
        Ok(lua)
    }

    pub fn lua(&self) -> &Lua {
        &self.lua
    }

    pub fn register_fn<F>(&self, f: F) -> Result<()>
    where
        F: Fn(&Lua) -> Result<()>,
    {
        f(&self.lua)?;
        Ok(())
    }

    fn task_defines(&self) -> Result<()> {
        let tasks = self.tasks.clone();

        self.lua.context::<_, Result<(), TaskError>>(|lua_ctx| {
            let globals = lua_ctx.globals();
            let task_table = lua_ctx.create_table()?;
            lua_ctx.set_named_registry_value("tasks", task_table)?;
            let task_fn = lua_ctx.create_function(
                move |ctx, (task_name, dependencies, f): (String, Vec<String>, rlua::Function)| {
                    let mut tasks = tasks.lock().unwrap();
                    tasks.push(TaskDefinition::new(task_name.clone(), dependencies));
                    let table: Table = ctx.named_registry_value("tasks")?;
                    table.set(task_name, f)?;
                    ctx.set_named_registry_value("tasks", table)?;
                    Ok(())
                },
            )?;

            globals.set("task", task_fn)?;
            Ok(())
        })?;
        Ok(())
    }

    fn eval_string(&self, src: &str) -> Result<()> {
        self.lua.context::<_, Result<(), TaskError>>(|lua_ctx| {
            lua_ctx.load(&src).exec()?;
            Ok(())
        })?;
        Ok(())
    }

    pub fn eval(self, src: &str) -> Result<EvaluatedLuaState> {
        self.eval_string(src)?;
        let tasks = self.tasks.lock().unwrap();
        let graph = TaskGraphState::from_tasks(tasks.to_vec())?;
        Ok(EvaluatedLuaState {
            lua: self.lua,
            graph,
        })
    }
}

pub struct EvaluatedLuaState {
    lua: Lua,
    graph: TaskGraphState,
}

impl EvaluatedLuaState {
    pub fn execution_ordering(&self, tasks: &[TaskRef]) -> Result<Vec<&TaskDefinition>, TaskError> {
        Ok(self.graph.execution_for_tasks(tasks)?)
    }

    pub fn execute(&self, tasks: &[TaskRef]) -> Result<()> {
        self.lua.context::<_, Result<(), TaskError>>(|lua_ctx| {
            let task_table: Table = lua_ctx.named_registry_value("tasks")?;
            let ordering = self.execution_ordering(tasks)?;
            for task in ordering {
                println!("--- Executing {}", &task.name());
                let f: Function = task_table.get(task.name().as_ref())?;
                f.call(())?;
            }
            Ok(())
        })?;
        Ok(())
    }
}
