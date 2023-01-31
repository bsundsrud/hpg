use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
};

use crate::{
    error::{self, TaskError},
    tasks::{TaskDefinition, TaskGraphState, TaskRef, TaskResult},
    Result, WRITER,
};
use mlua::{Function, Lua, LuaOptions, Table, Value, Variadic};

pub struct LuaState {
    lua: Lua,
    tasks: Arc<Mutex<Vec<TaskDefinition>>>,
}

fn std_lib() -> mlua::StdLib {
    use mlua::StdLib;
    StdLib::TABLE | StdLib::STRING | StdLib::UTF8 | StdLib::MATH | StdLib::PACKAGE
}

impl LuaState {
    pub fn new() -> Result<Self> {
        let lua = Self {
            lua: Lua::new_with(std_lib(), LuaOptions::new())
                .map_err(|e| error::TaskError::LuaError(e))?,
            tasks: Arc::new(Mutex::new(Vec::new())),
        };
        lua.task_defines()?;
        Ok(lua)
    }

    pub fn register_fn<F>(&self, f: F) -> Result<()>
    where
        F: Fn(&Lua) -> Result<(), TaskError>,
    {
        f(&self.lua)?;
        Ok(())
    }

    fn task_defines(&self) -> Result<(), TaskError> {
        let tasks = self.tasks.clone();
        let globals = self.lua.globals();
        let task_table = self.lua.create_table()?;
        self.lua.set_named_registry_value("tasks", task_table)?;
        let task_fn = self.lua.create_function(
            move |ctx,
                  (task_name, dependencies_or_f, maybe_f): (
                String,
                Value,
                Option<mlua::Function>,
            )| {
                let table: Table = ctx.named_registry_value("tasks")?;
                match (dependencies_or_f, maybe_f) {
                    (Value::Table(t), Some(f)) => {
                        let mut tasks = tasks.lock().unwrap();
                        let dependencies =
                            t.sequence_values().collect::<Result<Vec<String>, _>>()?;
                        tasks.push(TaskDefinition::new(task_name.clone(), dependencies));
                        table.set(task_name, f)?;
                    }
                    (Value::String(s), Some(f)) => {
                        let mut tasks = tasks.lock().unwrap();
                        tasks.push(TaskDefinition::new(
                            task_name.clone(),
                            vec![s.to_str().unwrap().into()],
                        ));
                        table.set(task_name, f)?;
                    }
                    (Value::Function(f), None) => {
                        let mut tasks = tasks.lock().unwrap();
                        tasks.push(TaskDefinition::new(task_name.clone(), Vec::new()));
                        table.set(task_name, f)?;
                    }
                    (Value::Table(t), None) => {
                        let mut tasks = tasks.lock().unwrap();
                        let dependencies =
                            t.sequence_values().collect::<Result<Vec<String>, _>>()?;
                        tasks.push(TaskDefinition::new(task_name.clone(), dependencies));
                    }
                    (Value::String(s), None) => {
                        let mut tasks = tasks.lock().unwrap();
                        tasks.push(TaskDefinition::new(
                            task_name.clone(),
                            vec![s.to_str().unwrap().into()],
                        ));
                    }
                    _ => {
                        return Err(error::action_error("Invalid signature for task() function"));
                    }
                }
                ctx.set_named_registry_value("tasks", table)?;
                Ok(())
            },
        )?;

        globals.set("task", task_fn)?;

        let targets: Vec<String> = Vec::new();
        self.lua.set_named_registry_value("targets", targets)?;
        let target_fn = self.lua.create_function(|ctx, tasks: Variadic<String>| {
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
    }

    fn eval_string(&self, src: &str) -> Result<(), TaskError> {
        self.lua.load(src).exec()?;
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

    pub fn execute(
        &self,
        tasks: &[TaskRef],
        run_targets: bool,
        show_plan: bool,
    ) -> Result<(), TaskError> {
        let task_table: Table = self.lua.named_registry_value("tasks")?;
        let ordering = if run_targets {
            let targets: Vec<String> = self.lua.named_registry_value("targets")?;
            let mut targets: Vec<TaskRef> = targets.into_iter().map(|s| s.into()).collect();
            targets.extend(tasks.into_iter().map(|t| t.clone()));
            self.execution_ordering(&targets)?
        } else {
            self.execution_ordering(tasks)?
        };
        let _guard = WRITER.enter("tasks");
        let mut results: HashMap<TaskRef, TaskResult> = HashMap::new();

        for (i, task) in ordering.iter().enumerate() {
            if show_plan {
                WRITER.write(format!("{}: {}", i + 1, task.name().as_ref()));
                continue;
            }
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
                    Ok(mlua::Value::UserData(ud)) => {
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
                    Err(mlua::Error::CallbackError { traceback, cause }) => {
                        if let mlua::Error::ExternalError(ref e) = *cause.clone() {
                            WRITER.write(format!("{}\n{}", e, traceback));
                            WRITER.write(format!("Source: {:?}", e.source()))
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
    }
}
