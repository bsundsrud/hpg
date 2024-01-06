use std::collections::{HashMap, HashSet};

use petgraph::graph::DiGraph;
use petgraph::prelude::*;

use super::{Task, TaskHandle, TaskRegistry};

pub type TaskIdx = NodeIndex<u32>;
pub type TaskGraph = DiGraph<Task, (), u32>;

pub struct GraphState {
    dag: TaskGraph,
    id_graph_map: HashMap<TaskHandle, TaskIdx>,
}

impl GraphState {
    pub fn from_registry(registry: TaskRegistry) -> Self {
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
                dep_map.entry(*idx).or_default().push(*dep_idx);
            }
        }

        for (idx, deps) in dep_map {
            for dep_idx in deps {
                dag.add_edge(idx, dep_idx, ());
            }
        }

        Self { dag, id_graph_map }
    }

    // Determine all dependent tasks for the given task
    // Order will start with deepest dependency and end with the given task
    fn dfs_post(&self, task: TaskIdx) -> Vec<TaskIdx> {
        let mut dfs = DfsPostOrder::new(&self.dag, task);
        let mut res = Vec::new();
        while let Some(nx) = dfs.next(&self.dag) {
            res.push(nx);
        }
        res
    }

    pub fn direct_parents(&self, task: TaskHandle) -> Vec<TaskHandle> {
        let mut res = Vec::new();
        let idx = self.id_graph_map.get(&task);
        if let Some(&idx) = idx {
            for i in self.dag.neighbors_directed(idx, Direction::Outgoing) {
                res.push(self.dag.node_weight(i).unwrap().id);
            }
        }
        res
    }

    // Find execution plan for a given task.  May contain duplicates if there is a diamond
    // in the digraph for a given task.
    fn execution_for_task(&self, task: TaskHandle) -> Vec<TaskHandle> {
        let start = self.id_graph_map.get(&task).unwrap();

        self.dfs_post(*start)
            .into_iter()
            .map(|idx| self.dag.node_weight(idx).unwrap().id)
            .collect()
    }

    /// Find the safe-order execution plan for the given handles.
    /// Order is from deepest dependency to top-level.
    /// Duplicates are removed such that tasks will retain their highest priority in the execution order.
    pub fn execution_for_tasks(&self, tasks: &[TaskHandle]) -> Vec<TaskHandle> {
        let mut uniques = HashSet::new();
        let mut execution: Vec<TaskHandle> = tasks
            .iter()
            .flat_map(|task| self.execution_for_task(*task))
            .collect();

        execution.retain(|t| uniques.insert(*t));
        execution
    }
}
