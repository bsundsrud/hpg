use std::{
    borrow::Cow,
    fmt::Arguments,
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};

use console::{style, Term};
use indicatif::{HumanDuration, MultiProgress, ProgressBar, ProgressStyle};
use lazy_static::lazy_static;

pub struct PrettyTracker {
    console: Term,
    bars: MultiProgress,
    run_bar: Mutex<Option<ProgressBar>>,
    current_task: Mutex<Option<String>>,
    started: Mutex<Option<Instant>>,
}

impl PrettyTracker {
    pub fn new() -> Self {
        let bars = MultiProgress::new();
        bars.set_alignment(indicatif::MultiProgressAlignment::Top);
        Self {
            console: Term::stdout(),
            bars,
            run_bar: Mutex::new(None),
            current_task: Mutex::new(None),
            started: Mutex::new(None),
        }
    }

    pub fn println(&self, args: Arguments) {
        self.bars.suspend(|| {
            self.console.write_line(&args.to_string()).unwrap();
        });
    }

    pub fn indent_println(&self, indent: usize, args: Arguments) {
        let s = args.to_string();
        let indent_str = " ".repeat(indent * 2);
        let mut lines = Vec::new();
        for line in s.lines() {
            lines.push(format!("{}{}", indent_str, line));
        }

        let output = lines.join("\n");
        self.bars
            .suspend(|| self.console.write_line(&output).unwrap());
    }

    pub fn task<S: Into<String>>(&self, msg: S) {
        let m = msg.into();
        *self.current_task.lock().unwrap() = Some(m.clone());
        if let Some(rb) = &*self.run_bar.lock().unwrap() {
            rb.set_message(m);
        }
    }

    pub fn run(&self, count: u64) {
        let mut rb = self.run_bar.lock().unwrap();
        if rb.is_none() {
            let bar = ProgressBar::new(count).with_style(
                ProgressStyle::with_template("[{pos}/{len}] ({elapsed}) {spinner} {msg}").unwrap(),
            );

            bar.enable_steady_tick(Duration::from_millis(200));
            self.bars.add(bar.clone());
            *rb = Some(bar);
        }
        *self.started.lock().unwrap() = Some(Instant::now());
    }

    pub fn task_success(&self) {
        let mut task = self.current_task.lock().unwrap();
        if let Some(t) = &*task {
            let _ = self
                .bars
                .println(format!("{} {}", style("✓ SUCCESS").green(), t));
        }
        *task = None;
        if let Some(rb) = &*self.run_bar.lock().unwrap() {
            rb.inc(1);
        }
    }

    pub fn task_skip(&self) {
        let mut task = self.current_task.lock().unwrap();
        if let Some(t) = &*task {
            let _ = self
                .bars
                .println(format!("{} {}", style("⧖ SKIPPED").cyan(), t));
        }
        *task = None;
        if let Some(rb) = &*self.run_bar.lock().unwrap() {
            rb.inc(1);
        }
    }

    pub fn task_fail(&self) {
        let mut task = self.current_task.lock().unwrap();
        if let Some(t) = &*task {
            let _ = self
                .bars
                .println(format!("{} {}", style("✗ FAILED").red(), t));
        }
        *task = None;
        if let Some(rb) = &*self.run_bar.lock().unwrap() {
            rb.inc(1);
        }
    }

    pub fn finish_success(&self) {
        let msg = if let Some(started) = &*self.started.lock().unwrap() {
            format!(
                "{} Done in {}.",
                style("✓").green(),
                HumanDuration(started.elapsed())
            )
        } else {
            format!("{} Done.", style("✓").green())
        };
        if let Some(rb) = &*self.run_bar.lock().unwrap() {
            rb.finish_and_clear();
            println!("{}", msg);
        }
    }

    pub fn finish_fail(&self) {
        let msg = if let Some(started) = &*self.started.lock().unwrap() {
            format!(
                "{} One or more tasks failed or were skipped. Done in {}.",
                style("✗").red(),
                HumanDuration(started.elapsed())
            )
        } else {
            format!(
                "{} One or more tasks failed or were skipped.",
                style("✗").red()
            )
        };
        if let Some(rb) = &*self.run_bar.lock().unwrap() {
            rb.finish_and_clear();
            println!("{}", msg);
        }
    }
}

lazy_static! {
    pub static ref TRACKER: PrettyTracker = PrettyTracker::new();
}
