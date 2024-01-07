use std::{
    fmt::{Arguments, Debug},
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc, Mutex, OnceLock, RwLock,
    },
    thread::JoinHandle,
    time::Duration,
};

use crossbeam::channel::{self, unbounded};
use futures_util::SinkExt;

use lazy_static::lazy_static;
use serde::{Deserialize, Serialize};
use tokio::net::UnixStream;
use tokio_util::codec::Framed;

use crate::remote::{
    codec::HpgCodec,
    messages::{ExecServerMessage, HpgMessage},
};

use self::local::PrettyTracker;
pub mod local;

pub trait Tracker {
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
    fn suspend_bars(&self);
    fn resume_bars(&self);
}

lazy_static! {
    pub static ref EVENT_SOURCE: OnceLock<EventSource> = OnceLock::new();
    pub static ref EVENT_SINK: OnceLock<EventSink> = OnceLock::new();
    static ref TRACKER_RUNTIME: tokio::runtime::Runtime =
        tokio::runtime::Builder::new_multi_thread()
            .enable_all()
            .build()
            .unwrap();
}

pub fn init(debug: bool) -> Result<SinkHandle, std::io::Error> {
    let (tx, rx) = unbounded();
    EVENT_SOURCE
        .set(EventSource::new(tx))
        .expect("Couldn't set event source");
    let sink = EventSink::new(rx);
    sink.set_debug(debug);
    EVENT_SINK.set(sink).expect("Couldn't set event sink");
    let handle = std::thread::Builder::new().spawn(|| {
        let e = EVENT_SINK.get().expect("Global Tracker not initialized");
        e.message_pump();
    })?;
    Ok(SinkHandle { handle })
}

pub fn tracker() -> &'static EventSource {
    EVENT_SOURCE.get().expect("Global tracker not initialized")
}

pub fn sink() -> &'static EventSink {
    EVENT_SINK.get().expect("Global tracker not initialized")
}

#[derive(Debug, Deserialize, Serialize, Clone, PartialEq, Eq)]
pub enum TrackerEvent {
    Println { msg: String, indent: Option<usize> },
    Debug(String),
    BatchStart(usize),
    BatchSuccess,
    BatchFail,
    TaskStart(String),
    TaskComplete,
    TaskFail,
    TaskSkip,
    ProgressStart(usize),
    ProgressInc(String),
    ProgressFinish(String),
    SuspendBars,
    ResumeBars,
    Exit,
}

#[derive(Debug)]
pub struct EventSource {
    tx: channel::Sender<TrackerEvent>,
}

impl EventSource {
    fn new(tx: channel::Sender<TrackerEvent>) -> EventSource {
        EventSource { tx }
    }

    pub fn finish(&self) {
        let _ = self.tx.send(TrackerEvent::Exit);
    }
}

impl Tracker for EventSource {
    fn debug_println(&self, args: Arguments) {
        let _ = self.tx.send(TrackerEvent::Debug(args.to_string()));
    }

    fn println(&self, args: Arguments) {
        let _ = self.tx.send(TrackerEvent::Println {
            msg: args.to_string(),
            indent: None,
        });
    }

    fn indent_println(&self, indent: usize, args: Arguments) {
        let _ = self.tx.send(TrackerEvent::Println {
            msg: args.to_string(),
            indent: Some(indent),
        });
    }

    fn run(&self, count: usize) {
        let _ = self.tx.send(TrackerEvent::BatchStart(count));
    }

    fn task(&self, task: String) {
        let _ = self.tx.send(TrackerEvent::TaskStart(task));
    }

    fn progressbar(&self, count: usize) {
        let _ = self.tx.send(TrackerEvent::ProgressStart(count));
    }

    fn progressbar_progress(&self, msg: String) {
        let _ = self.tx.send(TrackerEvent::ProgressInc(msg));
    }

    fn progressbar_finish(&self, msg: String) {
        let _ = self.tx.send(TrackerEvent::ProgressFinish(msg));
    }

    fn task_success(&self) {
        let _ = self.tx.send(TrackerEvent::TaskComplete);
    }

    fn task_skip(&self) {
        let _ = self.tx.send(TrackerEvent::TaskSkip);
    }

    fn task_fail(&self) {
        let _ = self.tx.send(TrackerEvent::TaskFail);
    }

    fn finish_success(&self) {
        let _ = self.tx.send(TrackerEvent::BatchSuccess);
    }

    fn finish_fail(&self) {
        let _ = self.tx.send(TrackerEvent::TaskFail);
    }

    fn suspend_bars(&self) {
        let _ = self.tx.send(TrackerEvent::SuspendBars);
    }

    fn resume_bars(&self) {
        let _ = self.tx.send(TrackerEvent::ResumeBars);
    }
}

pub struct SinkHandle {
    handle: JoinHandle<()>,
}

impl SinkHandle {
    pub fn finish(self) {
        tracker().finish();
        let _ = self.handle.join();
    }
}

#[derive(Debug)]
pub struct EventSink {
    rx: channel::Receiver<TrackerEvent>,
    output: Arc<RwLock<SinkType>>,
}

#[derive(Debug)]
pub enum SinkType {
    Local(PrettyTracker),
    Remote(RemoteWriter),
}

impl SinkType {
    fn debug(&self) -> bool {
        match self {
            SinkType::Local(l) => l.debug(),
            SinkType::Remote(r) => r.debug(),
        }
    }

    fn set_debug(&self, debug: bool) {
        match self {
            SinkType::Local(l) => l.set_debug(debug),
            SinkType::Remote(r) => r.set_debug(debug),
        }
    }
}

impl EventSink {
    pub fn new(rx: channel::Receiver<TrackerEvent>) -> EventSink {
        EventSink {
            rx,
            output: Arc::new(RwLock::new(SinkType::Local(PrettyTracker::new()))),
        }
    }

    /**
     * I hope busywaiting here works okay
     */
    fn wait_for_drain(&self) {
        while !self.rx.is_empty() {
            std::thread::sleep(Duration::from_millis(10));
        }
    }

    pub fn to_remote(&self, writer: Framed<UnixStream, HpgCodec<HpgMessage>>) {
        self.wait_for_drain();
        let output = &mut *self.output.write().unwrap();
        let debug = output.debug();
        let remote = RemoteWriter {
            out: Arc::new(Mutex::new(writer)),
            debug: AtomicBool::new(debug),
        };
        *output = SinkType::Remote(remote);
    }

    pub fn to_local(&self) -> Option<Framed<UnixStream, HpgCodec<HpgMessage>>> {
        self.wait_for_drain();
        let out = &mut *self.output.write().unwrap();
        let local = PrettyTracker::new();
        local.set_debug(out.debug());
        let sink = std::mem::replace(out, SinkType::Local(local));
        match sink {
            SinkType::Local(_) => None,
            SinkType::Remote(r) => {
                if let Ok(lock) = Arc::try_unwrap(r.out) {
                    let w = lock.into_inner().expect("Could not move out of mutex");
                    Some(w)
                } else {
                    None
                }
            }
        }
    }

    fn message_pump(&self) {
        while let Ok(m) = self.rx.recv() {
            if m == TrackerEvent::Exit {
                return;
            }
            let out = &*self.output.read().unwrap();
            match out {
                SinkType::Local(l) => l.event(&m),
                SinkType::Remote(r) => r.event(&m),
            }
        }
    }

    pub fn set_debug(&self, debug: bool) {
        let out = &*self.output.write().unwrap();
        out.set_debug(debug);
    }
}

pub trait EventWriter: Send + Sync {
    fn event(&self, ev: &TrackerEvent);
    fn set_debug(&self, debug: bool);
    fn debug(&self) -> bool;
}

pub struct RemoteWriter {
    out: Arc<Mutex<Framed<UnixStream, HpgCodec<HpgMessage>>>>,
    debug: AtomicBool,
}

impl Debug for RemoteWriter {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("RemoteWriter")
            .field("debug", &self.debug)
            .finish()
    }
}

impl EventWriter for RemoteWriter {
    fn event(&self, ev: &TrackerEvent) {
        let w = &mut *self.out.lock().unwrap();
        println!("{:?}", ev);
        let _ = TRACKER_RUNTIME.block_on(async move {
            w.send(HpgMessage::ExecServer(ExecServerMessage::Event(ev.clone())))
                .await
        });
    }

    fn set_debug(&self, debug: bool) {
        self.debug.store(debug, Ordering::Relaxed);
    }

    fn debug(&self) -> bool {
        self.debug.load(Ordering::Relaxed)
    }
}

impl EventWriter for PrettyTracker {
    fn event(&self, ev: &TrackerEvent) {
        match ev {
            TrackerEvent::Println { msg, indent } => {
                if let Some(i) = indent {
                    self.indent_println(*i, msg);
                } else {
                    self.println(msg);
                }
            }
            TrackerEvent::Debug(msg) => {
                if self.debug() {
                    self.debug_println(msg);
                }
            }
            TrackerEvent::BatchStart(count) => self.run(*count),
            TrackerEvent::BatchSuccess => self.finish_success(),
            TrackerEvent::BatchFail => self.finish_fail(),
            TrackerEvent::TaskStart(s) => self.task(s.clone()),
            TrackerEvent::TaskComplete => self.task_success(),
            TrackerEvent::TaskFail => self.task_fail(),
            TrackerEvent::TaskSkip => self.task_skip(),
            TrackerEvent::ProgressStart(count) => self.progressbar(*count),
            TrackerEvent::ProgressInc(msg) => self.progressbar_progress(msg.clone()),
            TrackerEvent::ProgressFinish(msg) => self.progressbar_finish(msg.clone()),
            TrackerEvent::Exit => unreachable!("Exit should be handled in message pump"),
            TrackerEvent::SuspendBars => self.suspend(),
            TrackerEvent::ResumeBars => self.resume(),
        }
    }
    fn set_debug(&self, debug: bool) {
        self.set_debug(debug);
    }

    fn debug(&self) -> bool {
        self.debug()
    }
}
