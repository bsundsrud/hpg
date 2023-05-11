use anyhow::{anyhow, Result};
use mlua::{self, Function, Lua, Table, UserData, Value};
use std::{
    collections::HashMap,
    sync::{atomic::AtomicUsize, Arc, RwLock},
};

#[derive(Debug, Clone)]
pub struct Task {
    id: usize,
    deps: Vec<Task>,
}

impl Task {
    pub fn new(id: usize, deps: Vec<Task>) -> Task {
        Task { id, deps }
    }
}

impl UserData for Task {}

#[derive(Debug, Clone)]
pub struct TaskRegistry {
    next_id: Arc<AtomicUsize>,
    named: Arc<RwLock<HashMap<String, usize>>>,
    tasks: Arc<RwLock<HashMap<usize, Task>>>,
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

    pub fn register_name<S: Into<String>>(&self, id: usize, name: S) {
        let mut named = self.named.write().unwrap();
        named.insert(name.into(), id);
    }

    pub fn task_for_id(&self, id: usize) -> Option<Task> {
        self.tasks.read().unwrap().get(&id).cloned()
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

fn find_tasks(ctx: &Lua, registry: TaskRegistry) -> Result<()> {
    let globals = ctx.globals();
    for pair in globals.pairs() {
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

pub fn define_task_function(lua: &Lua, registry: TaskRegistry) -> Result<()> {
    let task_table = lua.create_table()?;
    lua.set_named_registry_value("tasks", task_table)?;

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

    lua.globals().set("task", f)?;
    Ok(())
}

pub fn exec(lua: &Lua, code: &str) -> Result<()> {
    let registry = TaskRegistry::new();
    define_task_function(&lua, registry.clone())?;

    lua.load(code).exec()?;
    find_tasks(&lua, registry.clone())?;

    //let task_table: Table = lua.named_registry_value("tasks")?;
    println!("All Tasks:");
    for t in registry.tasks() {
        println!("{:?}", t);
    }

    println!("Named Tasks:");
    for (name, t) in registry.named_tasks() {
        println!("{}: {:?}", name, t);
    }

    Ok(())
}
