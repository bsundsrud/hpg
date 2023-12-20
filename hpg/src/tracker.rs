use std::{
    fmt::Arguments,
    fs::File,
    io::Write,
    pin::Pin,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc, Mutex,
    },
    time::{Duration, Instant},
};

use console::{style, Term};
use indicatif::{HumanDuration, MultiProgress, ProgressBar, ProgressStyle};
use lazy_static::lazy_static;
use tokio::io::{Stdin, Stdout};

use crate::remote::{
    comms::SyncBus,
    messages::{ExecServerMessage, HpgMessage, ServerEvent},
};

pub struct Tracker {
    debug: AtomicBool,
    inner: Mutex<TrackerInner>,
}

impl Tracker {
    pub fn new_local() -> Self {
        Self {
            debug: AtomicBool::new(false),
            inner: Mutex::new(TrackerInner::Local(PrettyTracker::new())),
        }
    }

    pub fn into_remote(&self, bus: SyncBus<Stdin, Stdout>) {
        (*self.inner.lock().unwrap()) = TrackerInner::Remote(RemoteTracker::new(bus));
    }

    pub fn set_debug(&self, debug: bool) {
        self.debug.store(debug, Ordering::SeqCst)
    }

    pub fn debug_println(&self, args: Arguments) {
        if self.debug.load(Ordering::SeqCst) {
            match &*self.inner.lock().unwrap() {
                TrackerInner::Local(l) => l.debug_println(args),
                TrackerInner::Remote(r) => r.debug_println(args),
            }
        }
    }

    pub fn println(&self, args: Arguments) {
        match &*self.inner.lock().unwrap() {
            TrackerInner::Local(l) => l.println(args),
            TrackerInner::Remote(r) => r.println(args),
        }
    }

    pub fn indent_println(&self, indent: usize, args: Arguments) {
        match &*self.inner.lock().unwrap() {
            TrackerInner::Local(l) => l.indent_println(indent, args),
            TrackerInner::Remote(r) => r.indent_println(indent, args),
        }
    }

    pub fn task<S: Into<String>>(&self, msg: S) {
        match &*self.inner.lock().unwrap() {
            TrackerInner::Local(l) => l.task(msg),
            TrackerInner::Remote(r) => r.task(msg),
        }
    }

    pub fn run(&self, count: u64) {
        match &*self.inner.lock().unwrap() {
            TrackerInner::Local(l) => l.run(count),
            TrackerInner::Remote(r) => r.run(count),
        }
    }

    pub fn task_success(&self) {
        match &*self.inner.lock().unwrap() {
            TrackerInner::Local(l) => l.task_success(),
            TrackerInner::Remote(r) => r.task_success(),
        }
    }

    pub fn task_skip(&self) {
        match &*self.inner.lock().unwrap() {
            TrackerInner::Local(l) => l.task_skip(),
            TrackerInner::Remote(r) => r.task_skip(),
        }
    }

    pub fn task_fail(&self) {
        match &*self.inner.lock().unwrap() {
            TrackerInner::Local(l) => l.task_fail(),
            TrackerInner::Remote(r) => r.task_fail(),
        }
    }

    pub fn finish_success(&self) {
        match &*self.inner.lock().unwrap() {
            TrackerInner::Local(l) => l.finish_success(),
            TrackerInner::Remote(r) => r.finish_success(),
        }
    }

    pub fn finish_fail(&self) {
        match &*self.inner.lock().unwrap() {
            TrackerInner::Local(l) => l.finish_fail(),
            TrackerInner::Remote(r) => r.finish_fail(),
        }
    }
}

pub enum TrackerInner {
    Local(PrettyTracker),
    Remote(RemoteTracker),
}

pub struct RemoteTracker {
    bus: SyncBus<Stdin, Stdout>,
    runtime: tokio::runtime::Runtime,
}

impl RemoteTracker {
    fn new(bus: SyncBus<Stdin, Stdout>) -> Self {
        let runtime = tokio::runtime::Builder::new_multi_thread()
            .enable_all()
            .build()
            .unwrap();
        RemoteTracker { bus, runtime }
    }

    fn write(&self, msg: HpgMessage) {
        self.runtime.block_on(async move {
            Pin::new(&self.bus).tx(msg).await.unwrap();
        });
    }

    pub fn debug_println(&self, args: Arguments) {
        self.write(HpgMessage::Debug(args.to_string()));
    }

    pub fn println(&self, args: Arguments) {
        self.write(HpgMessage::ExecServer(ExecServerMessage::Println(
            args.to_string(),
        )));
    }

    pub fn indent_println(&self, indent: usize, args: Arguments) {
        let s = args.to_string();
        let indent_str = " ".repeat(indent * 2);
        let mut lines = Vec::new();
        for line in s.lines() {
            lines.push(format!("{}{}", indent_str, line));
        }

        let output = lines.join("\n");
        self.write(HpgMessage::ExecServer(ExecServerMessage::Println(output)));
    }

    pub fn task<S: Into<String>>(&self, msg: S) {
        self.write(HpgMessage::ExecServer(ExecServerMessage::Event(
            ServerEvent::TaskStart(msg.into()),
        )));
    }

    pub fn run(&self, count: u64) {
        self.write(HpgMessage::ExecServer(ExecServerMessage::Event(
            ServerEvent::BatchStart(count),
        )));
    }

    pub fn task_success(&self) {
        self.write(HpgMessage::ExecServer(ExecServerMessage::Event(
            ServerEvent::TaskSuccess,
        )));
    }

    pub fn task_skip(&self) {
        self.write(HpgMessage::ExecServer(ExecServerMessage::Event(
            ServerEvent::TaskSkip,
        )));
    }

    pub fn task_fail(&self) {
        self.write(HpgMessage::ExecServer(ExecServerMessage::Event(
            ServerEvent::TaskFail,
        )));
    }

    pub fn finish_success(&self) {
        self.write(HpgMessage::ExecServer(ExecServerMessage::Event(
            ServerEvent::BatchSuccess,
        )));
    }

    pub fn finish_fail(&self) {
        self.write(HpgMessage::ExecServer(ExecServerMessage::Event(
            ServerEvent::BatchFail,
        )));
    }
}

pub struct PrettyTracker {
    console: Term,
    bars: MultiProgress,
    run_bar: Mutex<Option<ProgressBar>>,
    current_task: Mutex<Option<String>>,
    started: Mutex<Option<Instant>>,
}

impl PrettyTracker {
    fn new() -> Self {
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

    pub fn debug_println(&self, args: Arguments) {
        self.bars.suspend(|| {
            self.console
                .write_line(&style(args.to_string()).yellow().dim().to_string())
                .unwrap();
        });
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
    pub static ref TRACKER: Tracker = Tracker::new_local();
}
