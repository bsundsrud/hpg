use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
};

use crate::{
    error::TaskError,
    tasks::{TaskDefinition, TaskGraphState, TaskRef, TaskResult},
    Result, WRITER,
};
use rlua::{Function, Lua, Table, Variadic};

pub struct LuaState {
    lua: Lua,
    tasks: Arc<Mutex<Vec<TaskDefinition>>>,
}

fn std_lib() -> rlua::StdLib {
    use rlua::StdLib;
    StdLib::BASE | StdLib::TABLE | StdLib::STRING | StdLib::UTF8 | StdLib::MATH | StdLib::PACKAGE
}

impl LuaState {
    pub fn new() -> Result<Self> {
        let lua = Self {
            lua: Lua::new_with(std_lib()),
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
                move |ctx,
                      (task_name, dependencies, maybe_f): (
                    String,
                    Vec<String>,
                    Option<rlua::Function>,
                )| {
                    let mut tasks = tasks.lock().unwrap();
                    tasks.push(TaskDefinition::new(task_name.clone(), dependencies));
                    let table: Table = ctx.named_registry_value("tasks")?;
                    if let Some(f) = maybe_f {
                        table.set(task_name, f)?;
                    }
                    ctx.set_named_registry_value("tasks", table)?;
                    Ok(())
                },
            )?;

            globals.set("task", task_fn)?;

            let targets: Vec<String> = Vec::new();
            lua_ctx.set_named_registry_value("targets", targets)?;
            let target_fn = lua_ctx.create_function(|ctx, tasks: Variadic<String>| {
                let mut targets: Vec<String> = ctx.named_registry_value("targets")?;

                let tasks: Vec<String> = tasks.into_iter().collect();

                for task in tasks {
                    if !targets.contains(&task) {
                        targets.push(task);
                    }
                }
                ctx.set_named_registry_value("targets", targets)?;
                Ok(())
            })?;
            globals.set("target", target_fn)?;
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

    pub fn execute(&self, tasks: &[TaskRef], run_targets: bool) -> Result<()> {
        self.lua.context::<_, Result<(), TaskError>>(|lua_ctx| {
            let task_table: Table = lua_ctx.named_registry_value("tasks")?;
            let ordering = if run_targets {
                let targets: Vec<String> = lua_ctx.named_registry_value("targets")?;
                let mut targets: Vec<TaskRef> = targets.into_iter().map(|s| s.into()).collect();
                targets.extend(tasks.into_iter().map(|t| t.clone()));
                self.execution_ordering(&targets)?
            } else {
                self.execution_ordering(tasks)?
            };
            let _guard = WRITER.enter("tasks");
            let mut results: HashMap<TaskRef, TaskResult> = HashMap::new();

            for task in ordering {
                WRITER.write(format!("task [ {} ]:", task.name().as_ref()));
                let _guard = WRITER.enter(task.name().as_ref());
                let mut parent_failed = false;

                for parent in self.graph.direct_parents(&task.name()) {
                    match results.get(parent).unwrap() {
                        TaskResult::Success => {}
                        TaskResult::Incomplete(_) => {
                            WRITER.write("SKIPPED");
                            parent_failed = true;
                            break;
                        }
                    }
                }
                if parent_failed {
                    results.insert(task.name().clone(), TaskResult::Incomplete(None));
                    continue;
                }

                let maybe_f: Option<Function> = task_table.get(task.name().as_ref())?;
                if let Some(f) = maybe_f {
                    match f.call(()) {
                        Ok(rlua::Value::UserData(ud)) => {
                            if ud.is::<TaskResult>() {
                                let tr: &TaskResult = &ud.borrow().unwrap();
                                if let TaskResult::Incomplete(_) = tr {
                                    WRITER.write("TASK INCOMPLETE");
                                }
                                results.insert(task.name().clone(), tr.clone());
                            }
                        }
                        Ok(_) => {
                            results.insert(task.name().clone(), TaskResult::Success);
                        }
                        Err(rlua::Error::CallbackError { traceback, cause }) => {
                            if let rlua::Error::ExternalError(ref e) = *cause.clone() {
                                WRITER.write(format!("{}\n{}", e, traceback));
                            } else {
                                WRITER.write(format!("{}\n{}", cause, traceback));
                            }
                            break;
                        }
                        Err(e) => return Err(e.into()),
                    }
                } else {
                    results.insert(task.name().clone(), TaskResult::Success);
                }
            }
            if results.into_values().any(|r| r.incomplete()) {
                return Err(TaskError::SkippedTask);
            }
            Ok(())
        })?;
        Ok(())
    }
}
