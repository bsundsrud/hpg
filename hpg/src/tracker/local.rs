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

use super::Tracker;

pub struct PrettyTracker {
    console: Term,
    bars: MultiProgress,
    run_bar: Mutex<Option<ProgressBar>>,
    current_task: Mutex<Option<String>>,
    started: Mutex<Option<Instant>>,
    debug: AtomicBool,
}

impl PrettyTracker {
    pub(crate) fn new() -> Self {
        let bars = MultiProgress::new();
        bars.set_alignment(indicatif::MultiProgressAlignment::Top);
        Self {
            console: Term::stdout(),
            bars,
            run_bar: Mutex::new(None),
            current_task: Mutex::new(None),
            started: Mutex::new(None),
            debug: AtomicBool::new(false),
        }
    }
}

impl Tracker for PrettyTracker {
    fn set_debug(&self, debug: bool) {
        self.debug.store(debug, Ordering::SeqCst)
    }

    fn debug_println(&self, args: Arguments) {
        if self.debug.load(Ordering::Relaxed) {
            self.bars.suspend(|| {
                self.console
                    .write_line(&style(args.to_string()).yellow().dim().to_string())
                    .unwrap();
            });
        }
    }

    fn println(&self, args: Arguments) {
        self.bars.suspend(|| {
            self.console.write_line(&args.to_string()).unwrap();
        });
    }

    fn indent_println(&self, indent: usize, args: Arguments) {
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

    fn task(&self, msg: String) {
        *self.current_task.lock().unwrap() = Some(msg.clone());
        if let Some(rb) = &*self.run_bar.lock().unwrap() {
            rb.set_message(msg);
        }
    }

    fn progressbar(&self, count: usize) {
        let mut rb = self.run_bar.lock().unwrap();
        if rb.is_none() {
            let bar = ProgressBar::new(count as u64).with_style(
                ProgressStyle::with_template("[{pos}/{len}] ({elapsed}) {spinner} {msg}").unwrap(),
            );

            bar.enable_steady_tick(Duration::from_millis(200));
            self.bars.add(bar.clone());
            *rb = Some(bar);
        }
        *self.started.lock().unwrap() = Some(Instant::now());
    }

    fn progressbar_progress(&self, msg: String) {
        if let Some(rb) = &*self.run_bar.lock().unwrap() {
            rb.set_message(msg);
            rb.inc(1);
        }
    }

    fn progressbar_finish(&self, msg: String) {
        if let Some(rb) = &*self.run_bar.lock().unwrap() {
            rb.finish_with_message(msg);
        }
    }

    fn run(&self, count: usize) {
        let mut rb = self.run_bar.lock().unwrap();
        if rb.is_none() {
            let bar = ProgressBar::new(count as u64).with_style(
                ProgressStyle::with_template("[{pos}/{len}] ({elapsed}) {spinner} {msg}").unwrap(),
            );

            bar.enable_steady_tick(Duration::from_millis(200));
            self.bars.add(bar.clone());
            *rb = Some(bar);
        }
        *self.started.lock().unwrap() = Some(Instant::now());
    }

    fn task_success(&self) {
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

    fn task_skip(&self) {
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

    fn task_fail(&self) {
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

    fn finish_success(&self) {
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

    fn finish_fail(&self) {
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
