use std::{
    collections::{HashMap, HashSet, VecDeque},
    fmt::Display,
};

use anyhow::Result;
use petgraph::prelude::*;
use rlua::{self, Lua};
use structopt::StructOpt;
use thiserror::Error;

pub type TaskIdx = NodeIndex<u32>;
pub type TaskGraph = DiGraph<TaskDefinition, (), u32>;

#[derive(Debug, Error)]
pub enum TaskError {
    #[error("Cycle detected involving {0}")]
    CycleError(TaskRef),
    #[error("Unknown task {0}")]
    UnknownTask(TaskRef),
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
pub struct TaskDefinition {
    name: TaskRef,
    dependencies: Vec<TaskRef>,
}

impl TaskDefinition {
    pub fn new<S: Into<TaskRef>>(name: S, dependencies: Vec<S>) -> Self {
        Self {
            name: name.into(),
            dependencies: dependencies.into_iter().map(|s| s.into()).collect(),
        }
    }
}

pub struct TaskGraphState {
    dag: TaskGraph,
    ref_to_nodes: HashMap<TaskRef, TaskIdx>,
}

impl TaskGraphState {
    pub fn from_tasks(tasks: Vec<TaskDefinition>) -> Result<Self> {
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

    pub fn execution_for_task(&self, task: &TaskRef) -> Result<Vec<&TaskDefinition>> {
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

    pub fn execution_for_tasks(&self, tasks: Vec<TaskRef>) -> Result<Vec<&TaskDefinition>> {
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

fn create_lua() -> Lua {
    let lua = Lua::new();
    lua.context(|lua_ctx| {
        let task_fn = lua_ctx
            .create_function(
                |_, (task_name, dependencies, f): (String, Vec<String>, rlua::Function)| Ok(()),
            )
            .unwrap();
        let globals = lua_ctx.globals();
        globals.set("task", task_fn).unwrap();
    });
    lua
}

#[derive(Debug, StructOpt)]
#[structopt(name = "hpg", about = "config management tool")]
struct Opt {
    #[structopt(name = "TARGETS")]
    targets: Vec<String>,
}

fn main() -> Result<()> {
    let opt = Opt::from_args();

    let tasks = vec![
        TaskDefinition::new("A", Vec::new()),
        TaskDefinition::new("B", vec!["A", "F"]),
        TaskDefinition::new("C", vec!["A"]),
        TaskDefinition::new("D", vec!["B"]),
        TaskDefinition::new("E", vec!["D", "C"]),
        TaskDefinition::new("F", vec![]),
        TaskDefinition::new("G", vec![]),
        TaskDefinition::new("H", vec!["G"]),
    ];

    let state = TaskGraphState::from_tasks(tasks)?;

    let task_refs = opt.targets.into_iter().map(TaskRef::new).collect();
    let ordering = state.execution_for_tasks(task_refs)?;
    for task in ordering {
        println!("{}", task.name);
    }
    Ok(())
}
