use std::{collections::HashMap, fmt::Display};

use crate::{
    debug_output, indent_output, output,
    tracker::{self, Tracker},
    Result,
};
use anyhow::anyhow;
use console::style;
use mlua::{self, FromLua, Function, Lua, LuaOptions, Table, UserData, Value, Variadic};

use crate::error::TaskError;
pub mod graph;
pub mod vars;
pub use vars::Variables;
pub mod registry;
use self::{graph::GraphState, registry::TaskRegistry};

#[derive(Debug, Copy, Clone, Hash, Eq, PartialEq, Ord, PartialOrd)]
pub struct TaskHandle(usize);

impl Display for TaskHandle {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct Task {
    pub id: TaskHandle,
    description: String,
    deps: Vec<Task>,
}

impl Task {
    pub fn new(id: usize, description: String, deps: Vec<Task>) -> Task {
        Task {
            id: TaskHandle(id),
            description,
            deps,
        }
    }

    pub fn description(&self) -> &str {
        &self.description
    }
}

impl<'lua> FromLua<'lua> for Task {
    fn from_lua(value: Value<'lua>, _lua: &'lua Lua) -> mlua::Result<Self> {
        match value {
            Value::UserData(ud) => {
                if ud.is::<Task>() {
                    let t: &Task = &*ud.borrow::<Task>()?;
                    Ok(t.clone())
                } else {
                    Err(mlua::Error::runtime("UserData was not of type Task"))
                }
            }
            _ => Err(mlua::Error::runtime(
                "Only UserData can be converted to a Task",
            )),
        }
    }
}

impl UserData for Task {}

#[derive(Debug, Clone)]
pub enum TaskResult {
    Success,
    Incomplete(Option<String>),
}

impl TaskResult {
    #[allow(dead_code)]
    pub fn succeeded(&self) -> bool {
        match self {
            TaskResult::Success => true,
            TaskResult::Incomplete(_) => false,
        }
    }

    pub fn incomplete(&self) -> bool {
        match self {
            TaskResult::Success => false,
            TaskResult::Incomplete(_) => true,
        }
    }
}

impl UserData for TaskResult {}

fn std_lib() -> mlua::StdLib {
    use mlua::StdLib;
    StdLib::TABLE | StdLib::STRING | StdLib::UTF8 | StdLib::MATH | StdLib::PACKAGE
}

fn find_tasks(table: Table<'_>, registry: &TaskRegistry) -> Result<(), mlua::Error> {
    for pair in table.pairs() {
        let (name, val): (String, Value) = pair?;
        match val {
            Value::UserData(ud) => {
                if ud.is::<Task>() {
                    let ts: &Task = &*ud.borrow::<Task>()?;
                    registry.register_name(ts.id, name);
                }
            }
            _ => continue,
        }
    }
    Ok(())
}

pub struct LuaState {
    lua: Lua,
    registry: TaskRegistry,
}

impl LuaState {
    pub fn new() -> Result<Self> {
        let lua = Lua::new_with(std_lib(), LuaOptions::new()).unwrap();
        let registry = TaskRegistry::new();

        Ok(Self { lua, registry })
    }

    pub fn register_fn<F>(&self, f: F) -> Result<()>
    where
        F: Fn(&Lua) -> Result<(), TaskError>,
    {
        f(&self.lua)?;
        Ok(())
    }

    fn define_task_function(&self) -> Result<(), TaskError> {
        let task_table = self.lua.create_table()?;
        let lua = &self.lua;
        lua.set_named_registry_value("tasks", task_table)?;
        let registry = self.registry.clone();
        let f = lua.create_function(
            move |ctx, (desc, deps_or_f, maybe_f): (String, Value, Option<Function>)| {
                let mut task_deps = Vec::new();
                let mut task_fn = None;
                // Handle first argument to task() function
                match deps_or_f {
                    // Task deps table must be a sequence of UserData, and the UserData must be a Task
                    Value::Table(t) => {
                        let deps: Vec<Value> =
                            t.sequence_values().collect::<Result<Vec<Value>, _>>()?;
                        for dep in deps {
                            match dep {
                                Value::UserData(ud) => {
                                    if ud.is::<Task>() {
                                        let t: &Task = &*ud.borrow::<Task>()?;
                                        task_deps.push(t.clone());
                                    } else {
                                        return Err(mlua::Error::external(anyhow!(
                                            "Task dependencies must be a task or sequence of tasks"
                                        )));
                                    }
                                }
                                _ => {
                                    return Err(mlua::Error::external(anyhow!(
                                        "Invalid signature for task() function, in dep table"
                                    )));
                                }
                            }
                        }
                    }
                    // Single userdata values must be a task
                    Value::UserData(ud) => {
                        if ud.is::<Task>() {
                            let ts: &Task = &*ud.borrow::<Task>()?;
                            task_deps.push(ts.clone());
                        } else {
                            return Err(mlua::Error::external(anyhow!(
                                "Task dependencies must be a task or sequence of tasks"
                            )));
                        }
                    }
                    // No dependencies, only a task function
                    Value::Function(f) => {
                        task_fn = Some(f);
                    }
                    _ => {
                        return Err(mlua::Error::external(anyhow!(
                            "Invalid signature for task() function, second param not table, task, or function: {:?}", deps_or_f 
                        )))
                    }
                };

                if let Some(f) = maybe_f {
                    if task_fn.is_some() {
                        // This means the second argument was also a function, this is invalid.
                        return Err(mlua::Error::external(anyhow!(
                            "Invalid signature for task() function, two functions"
                        )));
                    }
                    task_fn = Some(f);
                }

                let task_table: Table = ctx.named_registry_value("tasks")?;
                let i = registry.next_id();
                if let Some(f) = task_fn {
                    task_table.set(i, f)?;
                }
                ctx.set_named_registry_value("tasks", task_table)?;

                let task = Task::new(i, desc, task_deps);
                debug_output!("Registered task '{}'", task.description());
                registry.register_task(task.clone());
                Ok(task)
            },
        )?;

        self.lua.globals().set("task", f)?;
        Ok(())
    }

    fn define_target_function(&self) -> Result<(), TaskError> {
        let targets: Vec<String> = Vec::new();
        self.lua.set_named_registry_value("targets", targets)?;
        let registry = self.registry.clone();
        let target_fn = self
            .lua
            .create_function(move |ctx, tasks: Variadic<Value>| {
                // At call time, make sure to gather all named tasks to have up-to-date availability
                find_tasks(ctx.globals(), &registry)?;
                let mut targets: Vec<Task> = ctx.named_registry_value("targets")?;
                for t in tasks.iter() {
                    match t {
                        Value::String(s) => {
                            if let Some(t) = registry.task_for_name(s.to_str().unwrap()) {
                                if !targets.contains(&t) {
                                    targets.push(t);
                                }
                            } else {
                                return Err(mlua::Error::runtime(format!(
                                    "Unknown task '{}'",
                                    s.to_string_lossy()
                                )));
                            }
                        }
                        Value::UserData(ud) => {
                            if ud.is::<Task>() {
                                let task: &Task = &ud.borrow().unwrap();
                                if !targets.contains(task) {
                                    targets.push(task.clone());
                                }
                            } else {
                                return Err(mlua::Error::runtime(
                                    "Invalid argument type to target()",
                                ));
                            }
                        }
                        _ => {
                            return Err(mlua::Error::runtime("Invalid argument type to target()"));
                        }
                    }
                }
                ctx.set_named_registry_value("targets", targets)?;
                Ok(())
            })?;
        self.lua.globals().set("target", target_fn)?;

        Ok(())
    }

    fn find_tasks(&self) -> Result<(), TaskError> {
        let globals = self.lua.globals();
        find_tasks(globals, &self.registry)?;
        Ok(())
    }

    fn eval_string(&self, src: &str) -> Result<(), TaskError> {
        self.lua.load(src).exec()?;
        Ok(())
    }

    pub fn eval(self, src: &str, v: Variables) -> Result<EvaluatedLuaState> {
        self.define_task_function()?;
        self.define_target_function()?;
        self.lua
            .globals()
            .set("vars", v)
            .map_err(|e| TaskError::Action(format!("Couldn't set vars global: {}", e)))?;

        self.eval_string(src)?;
        self.find_tasks()?;
        let graph = GraphState::from_registry(self.registry.clone());
        Ok(EvaluatedLuaState {
            lua: self.lua,
            registry: self.registry,
            graph,
        })
    }
}

pub struct EvaluatedLuaState {
    lua: Lua,
    registry: TaskRegistry,
    graph: GraphState,
}

impl EvaluatedLuaState {
    pub fn execution_ordering(&self, tasks: &[TaskHandle]) -> Vec<TaskHandle> {
        self.graph.execution_for_tasks(tasks)
    }

    fn get_targets(&self, requested: &[&str]) -> Result<Vec<Task>, TaskError> {
        let mut requested_handles = Vec::new();

        for t in requested {
            if let Some(task) = self.registry.task_for_name(t) {
                requested_handles.push(task);
            } else {
                return Err(TaskError::Action(format!("Unknown task {}", t)));
            }
        }
        Ok(requested_handles)
    }

    fn get_default_targets(&self) -> Result<Vec<Task>, TaskError> {
        let targets: Vec<Task> = self.lua.named_registry_value("targets")?;
        Ok(targets)
    }

    pub fn available_targets(&self) -> Vec<(String, Task)> {
        self.registry.named_tasks().into_iter().collect()
    }

    pub fn execute(
        &self,
        tasks: &[&str],
        run_default_targets: bool,
        show_plan: bool,
    ) -> Result<(), TaskError> {
        let mut requested_tasks = self.get_targets(tasks)?;
        if run_default_targets {
            let defaults = self.get_default_targets()?;
            if !defaults.is_empty() {
                output!("{}", style("Default Targets").cyan());
                for t in defaults.iter() {
                    indent_output!(1, "{}", t.description);
                }
            }
            requested_tasks.extend(defaults);
        }
        let requested_handles: Vec<TaskHandle> =
            requested_tasks.into_iter().map(|t| t.id).collect();

        let ordering = self.execution_ordering(&requested_handles);
        if show_plan {
            output!("{}", style("Execution Plan").yellow());
            for (idx, handle) in ordering.into_iter().enumerate() {
                let t = self.registry.task_for_handle(handle);
                indent_output!(1, "{}. {}", idx + 1, t.description);
            }
            return Ok(());
        }

        let mut task_results: HashMap<TaskHandle, TaskResult> = HashMap::new();
        let task_table: Table = self.lua.named_registry_value("tasks")?;
        tracker::tracker().run(ordering.len());
        output!("{}", style("Execution").yellow());
        for task in ordering {
            let t = self.registry.task_for_handle(task);
            tracker::tracker().task(t.description.clone());
            output!("Task [ {} ]", style(t.description.clone()).cyan());
            let mut parent_failed = false;

            // Did all our parents run successfully?
            for parent in self.graph.direct_parents(task) {
                // unwrap is safe because we're guaranteed to execute parents first due to ordering
                match task_results.get(&parent).unwrap() {
                    TaskResult::Success => {}
                    TaskResult::Incomplete(_) => {
                        tracker::tracker().task_skip();
                        parent_failed = true;
                        break;
                    }
                }
            }
            // If a parent hasn't been run, we also need to skip
            if parent_failed {
                task_results.insert(task, TaskResult::Incomplete(None));
                continue;
            }

            let maybe_f: Option<Function> = task_table.get(task.0)?;
            if let Some(f) = maybe_f {
                match f.call(()) {
                    Ok(mlua::Value::UserData(ud)) => {
                        if ud.is::<TaskResult>() {
                            let tr: &TaskResult = &ud.borrow().unwrap();
                            if let TaskResult::Incomplete(_) = tr {
                                tracker::tracker().task_skip();
                            }
                            task_results.insert(task, tr.clone());
                        } else {
                            task_results.insert(task, TaskResult::Success);
                        }
                    }
                    Ok(_) => {
                        tracker::tracker().task_success();
                        task_results.insert(task, TaskResult::Success);
                    }
                    Err(mlua::Error::CallbackError { traceback, cause }) => {
                        if let mlua::Error::ExternalError(ref e) = *cause.clone() {
                            output!("{}\n{}", e, traceback);
                            output!("Source: {:?}", e.source())
                        } else {
                            output!("{}\n{}", cause, traceback);
                        }
                        tracker::tracker().task_fail();
                        task_results
                            .insert(task, TaskResult::Incomplete(Some("Error".to_string())));
                        break;
                    }
                    Err(e) => return Err(e.into()),
                }
            } else {
                task_results.insert(task, TaskResult::Success);
            }
        }
        if task_results.into_values().any(|r| r.incomplete()) {
            tracker::tracker().finish_fail();
            return Err(TaskError::SkippedTask);
        }
        tracker::tracker().finish_success();
        Ok(())
    }
}
