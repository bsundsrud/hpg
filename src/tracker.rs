use std::{
    sync::{Arc, Mutex},
    time::Duration,
};

use console::Term;
use indicatif::{MultiProgress, ProgressBar};
use lazy_static::lazy_static;

pub struct TaskContext {
    pb: ProgressBar,
    bars: MultiProgress,
    description: String,
}

impl TaskContext {
    fn new(bars: MultiProgress, description: &str) -> Self {
        let pb = ProgressBar::new_spinner();
        pb.enable_steady_tick(Duration::from_millis(200));
        pb.set_message(description.to_string());
        Self {
            pb,
            bars,
            description: description.to_string(),
        }
    }

    pub fn println<S: AsRef<str>>(&self, msg: S) {
        self.bars
            .suspend(|| console::Term::stdout().write_line(msg.as_ref()))
            .unwrap();
    }
}

impl Drop for TaskContext {
    fn drop(&mut self) {
        self.bars.remove(&self.pb);
    }
}

pub struct RunContext {
    pb: ProgressBar,
}

impl RunContext {
    fn new(tasks: u64) -> Self {
        let pb = ProgressBar::new(tasks);
        Self { pb }
    }
}

impl Drop for RunContext {
    fn drop(&mut self) {
        println!("RC Drop");
    }
}

pub struct PrettyTracker {
    console: Term,
    bars: MultiProgress,
}

impl PrettyTracker {
    pub fn new() -> Self {
        Self {
            console: Term::stdout(),
            bars: MultiProgress::new(),
        }
    }

    pub fn println<S: AsRef<str>>(&self, msg: S) {
        self.bars
            .suspend(|| self.console.write_line(msg.as_ref()))
            .unwrap();
    }
}

lazy_static! {
    pub static ref TRACKER: PrettyTracker = PrettyTracker::new();
    pub static ref OUTPUT: Output = Output::new();
}

pub struct Output {
    tracker: PrettyTracker,
    current_run: Mutex<Option<Arc<RunContext>>>,
    current_task: Mutex<Option<Arc<TaskContext>>>,
}

impl Output {
    fn new() -> Self {
        Self {
            tracker: PrettyTracker::new(),
            current_run: Mutex::new(None),
            current_task: Mutex::new(None),
        }
    }

    pub fn tracker(&self) -> &PrettyTracker {
        &self.tracker
    }

    pub fn println<S: AsRef<str>>(&self, msg: S) {
        self.tracker.println(msg)
    }

    pub fn run(&self, tasks: u64) -> Arc<RunContext> {
        let rc = RunContext::new(tasks);
        self.tracker.bars.add(rc.pb.clone());
        let arc = Arc::new(rc);
        *self.current_run.lock().unwrap() = Some(arc.clone());
        arc
    }

    pub fn task(&self, description: &str) -> Arc<TaskContext> {
        let tc = TaskContext::new(self.tracker.bars.clone(), description);
        self.tracker.bars.add(tc.pb.clone());
        let arc = Arc::new(tc);
        *self.current_task.lock().unwrap() = Some(arc.clone());
        arc
    }

    pub fn current_task(&self) -> Arc<TaskContext> {
        let what = &*self.current_task.lock().unwrap();
        if let Some(rc) = what {
            rc.clone()
        } else {
            panic!("no current task");
        }
    }
}
