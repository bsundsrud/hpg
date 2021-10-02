use std::{
    collections::{HashMap, HashSet, VecDeque},
    fmt::Display,
    sync::{Arc, Mutex},
};

use petgraph::prelude::*;
use rlua::{self, Function, Lua, Table};
use structopt::StructOpt;
use thiserror::Error;

pub type Result<T, E = TaskError> = std::result::Result<T, E>;
pub type TaskIdx = NodeIndex<u32>;
pub type TaskGraph<'lua> = DiGraph<TaskDefinition<'lua>, (), u32>;

#[derive(Debug, Error)]
pub enum TaskError {
    #[error("Cycle detected involving {0}")]
    CycleError(TaskRef),
    #[error("Unknown task {0}")]
    UnknownTask(TaskRef),
    #[error("Lua Error: {0}")]
    LuaError(#[from] rlua::Error),
}

#[derive(Debug, PartialEq, Eq, Hash, Clone)]
pub struct TaskRef(String);

impl TaskRef {
    pub fn new<S: Into<TaskRef>>(name: S) -> Self {
        name.into()
    }
}

impl From<String> for TaskRef {
    fn from(s: String) -> Self {
        TaskRef(s)
    }
}

impl From<&str> for TaskRef {
    fn from(s: &str) -> Self {
        TaskRef(s.to_string())
    }
}

impl Display for TaskRef {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

#[derive(Debug)]
pub struct TaskDefinition<'lua> {
    name: TaskRef,
    dependencies: Vec<TaskRef>,
    f: Function<'lua>,
}

impl<'lua> TaskDefinition<'lua> {
    pub fn new<S: Into<TaskRef>>(name: S, dependencies: Vec<S>, f: Function<'lua>) -> Self {
        Self {
            name: name.into(),
            dependencies: dependencies.into_iter().map(|s| s.into()).collect(),
            f,
        }
    }
}

pub struct TaskGraphState<'lua> {
    dag: TaskGraph<'lua>,
    ref_to_nodes: HashMap<TaskRef, TaskIdx>,
}

impl<'lua> TaskGraphState<'lua> {
    pub fn from_tasks(tasks: Vec<TaskDefinition<'lua>>) -> Result<Self> {
        let mut dag = TaskGraph::new();
        let mut idx_map: HashMap<TaskRef, TaskIdx> = HashMap::new();

        for task in tasks {
            let r = task.name.clone();
            let idx = dag.add_node(task);
            idx_map.insert(r, idx);
        }
        let mut dep_map: HashMap<TaskIdx, Vec<TaskIdx>> = HashMap::new();

        for n in dag.node_weights() {
            let idx = idx_map
                .get(&n.name)
                .ok_or_else(|| TaskError::UnknownTask(n.name.clone()))?;
            for dep_ref in n.dependencies.iter() {
                let dep_idx = idx_map
                    .get(dep_ref)
                    .ok_or_else(|| TaskError::UnknownTask(dep_ref.clone()))?;
                dep_map
                    .entry(*idx)
                    .or_insert_with(|| Vec::new())
                    .push(*dep_idx);
            }
        }

        for (idx, v) in dep_map.iter() {
            for dep_idx in v {
                dag.add_edge(*idx, *dep_idx, ());
            }
        }
        Ok(Self {
            dag,
            ref_to_nodes: idx_map,
        })
    }

    fn dfs_post(&self, task: TaskIdx) -> Vec<TaskIdx> {
        let mut dfs = DfsPostOrder::new(&self.dag, task);
        let mut res = Vec::new();
        while let Some(nx) = dfs.next(&self.dag) {
            res.push(nx);
        }
        res
    }

    pub fn execution_for_task(&self, task: &TaskRef) -> Result<Vec<&TaskDefinition<'lua>>> {
        let start_idx = self
            .ref_to_nodes
            .get(task)
            .ok_or_else(|| TaskError::UnknownTask(task.clone()))?;

        Ok(self
            .dfs_post(*start_idx)
            .into_iter()
            .map(|idx| self.dag.node_weight(idx).unwrap())
            .collect())
    }

    pub fn execution_for_tasks(&self, tasks: Vec<TaskRef>) -> Result<Vec<&TaskDefinition<'lua>>> {
        let mut uniques = HashSet::new();

        let mut execution: Vec<&TaskDefinition> = tasks
            .into_iter()
            .map(|task| self.execution_for_task(&task)) // Results in Vec<Result<Vec<_>>>
            .collect::<Result<Vec<_>>>()? // Transform to Result<Vec<Vec<_>>> and handle errors here
            .into_iter()
            .flatten() // Flatten the double-Vec and merge all execution chains
            .collect();

        // Execution is in topo-order for the given tree(s), if there's any overlap
        // between the given tasks we retain the entry that is higher in the execution order
        execution.retain(|e| uniques.insert(e.name.clone()));

        Ok(execution)
    }
}

fn create_lua(src: &str, requested_tasks: Vec<TaskRef>) -> Result<()> {
    let lua = Lua::new();
    lua.context::<_, Result<()>>(|lua_ctx| {
        let globals = lua_ctx.globals();
        let task_table = lua_ctx.create_table()?;
        globals.set("_tasks", task_table)?;
        let task_fn = lua_ctx.create_function_mut(
            |ctx, (task_name, dependencies, f): (String, Vec<String>, rlua::Function)| {
                let table: Table = ctx.globals().get("_tasks")?;
                let t = ctx.create_table()?;
                t.set("deps", dependencies)?;
                t.set("f", f)?;
                table.set(task_name, t)?;
                ctx.globals().set("_tasks", table)?;
                Ok(())
            },
        )?;
        // inject rust functions
        globals.set("task", task_fn)?;

        // eval root script

        lua_ctx.load(&src).exec()?;

        // Extract task data

        let task_table: Table = globals.get("_tasks")?;
        let mut tasks = Vec::new();
        for pair in task_table.pairs::<String, Table>() {
            let (task_name, data) = pair?;
            let deps = data.get("deps")?;
            let f = data.get("f")?;
            tasks.push(TaskDefinition::new(task_name, deps, f));
        }

        let task_state = TaskGraphState::from_tasks(tasks)?;
        let execution_ordering = task_state.execution_for_tasks(requested_tasks)?;

        // Execute
        for task in execution_ordering {
            println!("--- Executing {}", &task.name);
            task.f.call(())?;
        }
        Ok(())
    })?;
    Ok(())
}

#[derive(Debug, StructOpt)]
#[structopt(name = "hpg", about = "config management tool")]
struct Opt {
    #[structopt(name = "TARGETS")]
    targets: Vec<String>,
}

fn main() -> Result<()> {
    let opt = Opt::from_args();

    // let tasks = vec![
    //     TaskDefinition::new("A", Vec::new()),
    //     TaskDefinition::new("B", vec!["A", "F"]),
    //     TaskDefinition::new("C", vec!["A"]),
    //     TaskDefinition::new("D", vec!["B"]),
    //     TaskDefinition::new("E", vec!["D", "C"]),
    //     TaskDefinition::new("F", vec![]),
    //     TaskDefinition::new("G", vec![]),
    //     TaskDefinition::new("H", vec!["G"]),
    // ];

    // let state = TaskGraphState::from_tasks(tasks)?;

    let task_refs = opt.targets.into_iter().map(TaskRef::new).collect();
    let lua_code = r#"
task("foo", {}, function ()
  print "from foo"
end)

task("bar", {"foo"}, function()
  print "from bar"
end)

task("baz", {"foo"}, function()
  print "from baz"
end)

task("quux", {"bar", "baz"}, function()
  print "from quux"
end)
"#;
    create_lua(lua_code.into(), task_refs)?;
    // let ordering = state.execution_for_tasks(task_refs)?;
    // for task in ordering {
    //     println!("{}", task.name);
    // }
    Ok(())
}
