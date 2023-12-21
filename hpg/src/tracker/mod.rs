use std::{
    fmt::{Arguments, Debug},
    sync::{
        atomic::{AtomicBool, Ordering},
        Mutex, Once, OnceLock,
    },
    time::{Duration, Instant},
};

use console::{style, Term};
use indicatif::{HumanDuration, MultiProgress, ProgressBar, ProgressStyle};
use lazy_static::lazy_static;
pub mod local;
pub mod remote;

pub struct TrackerInner {
    tracker: Mutex<Box<dyn Tracker + Send + Sync>>,
}

impl Debug for TrackerInner {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("TrackerInner").finish()
    }
}

impl Tracker for TrackerInner {
    fn set_debug(&self, debug: bool) {
        self.tracker.lock().unwrap().set_debug(debug);
    }

    fn debug_println(&self, args: Arguments) {
        self.tracker.lock().unwrap().debug_println(args);
    }

    fn println(&self, args: Arguments) {
       self.tracker.lock().unwrap().println(args);
    }

    fn indent_println(&self, indent: usize, args: Arguments) {
        self.tracker.lock().unwrap().indent_println(indent, args);
    }

    fn run(&self, count: usize) {
        self.tracker.lock().unwrap().run(count);
    }

    fn task(&self, task: String) {
        self.tracker.lock().unwrap().task(task);
    }

    fn progressbar(&self, count: usize) {
        self.tracker.lock().unwrap().progressbar(count);
    }

    fn progressbar_progress(&self, msg: String) {
        self.tracker.lock().unwrap().progressbar_progress(msg);
    }

    fn progressbar_finish(&self, msg: String) {
        self.tracker.lock().unwrap().progressbar_finish(msg);
    }

    fn task_success(&self) {
        self.tracker.lock().unwrap().task_success();
    }

    fn task_skip(&self) {
        self.tracker.lock().unwrap().task_skip();
    }

    fn task_fail(&self) {
        self.tracker.lock().unwrap().task_fail();
    }

    fn finish_success(&self) {
        self.tracker.lock().unwrap().finish_success();
    }

    fn finish_fail(&self) {
        self.tracker.lock().unwrap().finish_fail();
    }
}

pub trait Tracker {
    fn set_debug(&self, debug: bool);
    fn debug_println(&self, args: Arguments);
    fn println(&self, args: Arguments);
    fn indent_println(&self, indent: usize, args: Arguments);
    fn run(&self, count: usize);
    fn task(&self, task: String);
    fn progressbar(&self, count: usize);
    fn progressbar_progress(&self, msg: String);
    fn progressbar_finish(&self, msg: String);
    fn task_success(&self);
    fn task_skip(&self);
    fn task_fail(&self);
    fn finish_success(&self);
    fn finish_fail(&self);
}


lazy_static! {
    pub static ref TRACKER: OnceLock<TrackerInner> = OnceLock::new();
}

pub fn init_local() -> &'static TrackerInner {
    let t = TrackerInner {
        tracker: Mutex::new(Box::new(local::PrettyTracker::new()))
    };
    TRACKER.set(t).expect("Global tracker initialized more than once");
    TRACKER.get().unwrap()
}

pub fn global() -> &'static TrackerInner {
    TRACKER.get().expect("Global tracker was not initialized")
}