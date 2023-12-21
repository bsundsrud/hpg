use std::{sync::{atomic::{AtomicBool, Ordering}, Arc, Mutex}, fmt::Arguments};

use futures_util::SinkExt;
use tokio::io::AsyncWrite;
use tokio_util::codec::FramedWrite;

use crate::remote::{codec::HpgCodec, messages::HpgMessage};

use super::Tracker;


pub struct RemoteTracker<W> {
    debug: AtomicBool,
    writer: Arc<Mutex<FramedWrite<W, HpgCodec<HpgMessage>>>>,
    runtime: tokio::runtime::Runtime,
}

impl<W> RemoteTracker<W>
where W: AsyncWrite + Unpin {
    pub fn new(writer: W) -> RemoteTracker<W> {
        let runtime = tokio::runtime::Builder::new_multi_thread().enable_all().build().unwrap();
        RemoteTracker {
            debug: AtomicBool::new(false),
            writer: Arc::new(Mutex::new(FramedWrite::new(writer, HpgCodec::new()))),
            runtime,
        }
    }

    fn send(&self, msg: HpgMessage) {
        let rt = self.runtime.handle();
        let w = self.writer.clone();
        let w = &mut *w.lock().unwrap();
        let _ = rt.enter();
        let _res = rt.block_on(async move {
            w.send(msg).await
        });
    }
}

impl<W> Tracker for RemoteTracker<W> 
where W: AsyncWrite + Unpin {
    fn set_debug(&self, debug: bool) {
        self.debug.store(debug, Ordering::Relaxed);
    }

    fn debug_println(&self, args: Arguments) {
        todo!()
    }

    fn println(&self, args: Arguments) {
        todo!()
    }

    fn indent_println(&self, indent: usize, args: Arguments) {
        todo!()
    }

    fn run(&self, count: usize) {
        todo!()
    }

    fn task(&self, task: String) {
        todo!()
    }

    fn progressbar(&self, count: usize) {
        todo!()
    }

    fn progressbar_progress(&self, msg: String) {
        todo!()
    }

    fn progressbar_finish(&self, msg: String) {
        todo!()
    }

    fn task_success(&self) {
        todo!()
    }

    fn task_skip(&self) {
        todo!()
    }

    fn task_fail(&self) {
        todo!()
    }

    fn finish_success(&self) {
        todo!()
    }

    fn finish_fail(&self) {
        todo!()
    }
}