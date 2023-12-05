use std::{
    collections::HashMap,
    sync::{atomic::AtomicUsize, Arc, RwLock},
};

use crate::debug_output;

use super::{Task, TaskHandle};

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

    pub fn register_name<S: Into<String>>(&self, id: TaskHandle, name: S) {
        let mut named = self.named.write().unwrap();
        let name = name.into();
        named.entry(name.clone()).or_insert_with(|| {
            debug_output!("Registered name {}", name);
            id
        });
    }

    pub fn task_for_handle(&self, id: TaskHandle) -> Task {
        self.tasks.read().unwrap().get(&id).unwrap().clone()
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
