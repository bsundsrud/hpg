use std::collections::HashMap;

use petgraph::prelude::*;
use rlua::{self, Lua};

pub type TaskIdx = NodeIndex<u32>;
pub type TaskGraph = DiGraph<TaskDefinition, (), u32>;

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

fn create_graph(tasks: Vec<TaskDefinition>) -> TaskGraph {
    let mut dag = TaskGraph::new();
    let mut idx_map: HashMap<TaskRef, TaskIdx> = HashMap::new();

    for task in tasks {
        let r = task.name.clone();
        let idx = dag.add_node(task);
        idx_map.insert(r, idx);
    }
    let mut dep_map: HashMap<TaskIdx, Vec<TaskIdx>> = HashMap::new();

    for n in dag.node_weights() {
        let idx = idx_map.get(&n.name).unwrap();
        for dep_ref in n.dependencies.iter() {
            let dep_idx = idx_map.get(dep_ref).unwrap();
            dep_map
                .entry(*idx)
                .or_insert_with(|| Vec::new())
                .push(*dep_idx);
        }
    }

    for (idx, v) in dep_map.iter() {
        for dep_idx in v {
            dag.add_edge(*dep_idx, *idx, ());
        }
    }
    dag
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

fn main() {
    let tasks = vec![
        TaskDefinition::new("A", Vec::new()),
        TaskDefinition::new("B", vec!["A"]),
        TaskDefinition::new("C", vec!["A"]),
        TaskDefinition::new("D", vec!["B"]),
        TaskDefinition::new("E", vec!["D", "C"]),
        TaskDefinition::new("F", vec![]),
        TaskDefinition::new("G", vec![]),
        TaskDefinition::new("H", vec!["G"]),
    ];

    let dag = create_graph(tasks);
    let res = petgraph::algo::toposort(&dag, None).unwrap();
    for idx in res {
        let task = &dag[idx];
        println!("{:?}", task.name);
    }
}
