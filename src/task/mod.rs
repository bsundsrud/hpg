use crate::{actions::util, error, Result};
use anyhow::anyhow;
use mlua::{
    self, chunk, Function, HookTriggers, Lua, LuaOptions, MetaMethod, Table, UserData, Value,
    Variadic,
};
use std::{
    collections::HashMap,
    sync::{atomic::AtomicUsize, Arc, RwLock},
};

use crate::error::TaskError;
pub mod graph;
pub mod vars;
pub use vars::Variables;

use self::graph::GraphState;

#[derive(Debug, Copy, Clone, Hash, Eq, PartialEq, Ord, PartialOrd)]
pub struct TaskHandle(usize);

#[derive(Debug, Clone)]
pub struct Task {
    id: TaskHandle,
    deps: Vec<Task>,
}

impl Task {
    pub fn new(id: usize, deps: Vec<Task>) -> Task {
        Task {
            id: TaskHandle(id),
            deps,
        }
    }
}

impl UserData for Task {}

#[derive(Debug, Clone)]
pub struct TaskRegistry {
    next_id: Arc<AtomicUsize>,
    named: Arc<RwLock<HashMap<String, TaskHandle>>>,
    tasks: Arc<RwLock<HashMap<TaskHandle, Task>>>,
}

impl TaskRegistry {
    pub fn new() -> Self {
        Self {
            next_id: Arc::new(AtomicUsize::new(1)),
            named: Arc::new(RwLock::new(HashMap::new())),
            tasks: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    pub fn register_task(&self, task: Task) {
        let mut tasks = self.tasks.write().unwrap();
        tasks.insert(task.id, task);
    }

    pub fn identify(&self, handle: TaskHandle) -> String {
        if let Some((name, _handle)) = self
            .named
            .read()
            .unwrap()
            .iter()
            .find(|(_k, v)| v.0 == handle.0)
        {
            return name.clone();
        } else {
            return format!("AnonymousTask({})", handle.0);
        }
    }

    pub fn register_name<S: Into<String>>(&self, id: TaskHandle, name: S) {
        let mut named = self.named.write().unwrap();
        named.insert(name.into(), id);
    }

    pub fn task_for_id(&self, id: usize) -> Option<Task> {
        self.tasks.read().unwrap().get(&TaskHandle(id)).cloned()
    }

    pub fn task_for_name(&self, name: &str) -> Option<Task> {
        if let Some(i) = self.named.read().unwrap().get(name) {
            self.tasks.read().unwrap().get(i).cloned()
        } else {
            None
        }
    }

    pub fn tasks(&self) -> Vec<Task> {
        self.tasks.read().unwrap().values().cloned().collect()
    }

    pub fn named_tasks(&self) -> HashMap<String, Task> {
        let tasks = self.tasks.read().unwrap();
        self.named
            .read()
            .unwrap()
            .iter()
            .map(|(name, id)| (name.clone(), tasks.get(id).cloned().unwrap()))
            .collect()
    }

    pub fn next_id(&self) -> usize {
        self.next_id
            .fetch_add(1, std::sync::atomic::Ordering::SeqCst)
    }
}

fn std_lib() -> mlua::StdLib {
    use mlua::StdLib;
    StdLib::TABLE | StdLib::STRING | StdLib::UTF8 | StdLib::MATH | StdLib::PACKAGE
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
            move |ctx, (deps_or_f, maybe_f): (Value, Option<Function>)| {
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
                                        "Invalid signature for task() function"
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
                            "Invalid signature for task() function"
                        )))
                    }
                };

                if let Some(f) = maybe_f {
                    if task_fn.is_some() {
                        // This means the second argument was also a function, this is invalid.
                        return Err(mlua::Error::external(anyhow!(
                            "Invalid signature for task() function"
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

                let task = Task::new(i, task_deps);
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
        self.lua.globals().set("target", target_fn)?;

        Ok(())
    }

    fn find_tasks(&self) -> Result<(), TaskError> {
        let globals = self.lua.globals();
        for pair in globals.pairs() {
            let (name, val): (String, Value) = pair?;
            match val {
                Value::UserData(ud) => {
                    if ud.is::<Task>() {
                        let ts: &Task = &*ud.borrow::<Task>()?;
                        self.registry.register_name(ts.id, name);
                    }
                }
                _ => continue,
            }
        }
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
            .map_err(|e| TaskError::ActionError(format!("Couldn't set vars global: {}", e)))?;

        self.eval_string(&src)?;
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

    pub fn execute(
        &self,
        tasks: &[&str],
        run_default_targets: bool,
        show_plan: bool,
    ) -> Result<(), TaskError> {
        let mut requested_handles = Vec::new();

        for t in tasks {
            if let Some(task) = self.registry.task_for_name(t) {
                requested_handles.push(task.id);
            } else {
                return Err(TaskError::ActionError(format!("Unknown task {}", t)));
            }
        }

        let ordering = self.execution_ordering(&requested_handles);
        if show_plan {
            for handle in ordering {
                println!("{}", self.registry.identify(handle));
            }
        }

        todo!()
    }
}
