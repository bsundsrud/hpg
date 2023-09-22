use std::collections::HashMap;

use petgraph::graph::DiGraph;
use petgraph::prelude::*;

use crate::error::TaskError;

use super::{Task, TaskRegistry};

pub type TaskIdx = NodeIndex<u32>;
pub type TaskGraph = DiGraph<Task, (), u32>;

pub struct GraphState {
    dag: TaskGraph,
    registry: TaskRegistry,
    id_graph_map: HashMap<usize, TaskIdx>,
}

impl GraphState {
    pub fn from_registry(registry: TaskRegistry) -> Result<Self, TaskError> {
        let mut dag = TaskGraph::new();
        let mut id_graph_map = HashMap::new();
        for task in registry.tasks() {
            let task_id = task.id;
            let idx = dag.add_node(task);
            id_graph_map.insert(task_id, idx);
        }

        let mut dep_map: HashMap<TaskIdx, Vec<TaskIdx>> = HashMap::new();

        // Unwraps are okay here because if the tasks DON'T exist, we already would
        // have failed to exec the lua for missing references
        for task in dag.node_weights() {
            let idx = id_graph_map
                .get(&task.id)
                .expect("graph/id map out of sync");
            for dep in task.deps.iter() {
                let dep_idx = id_graph_map
                    .get(&dep.id)
                    .expect("Dep not found in task map");
            }
        }

        Ok(Self {
            dag,
            registry,
            id_graph_map,
        })
    }
}
