use std::collections::VecDeque;
use std::fmt::Debug;
use std::io::Write;
use std::sync::{Arc, Mutex};

#[derive(Clone)]
pub enum Target {
    Stdout,
    Stderr,
    Writer(Arc<Mutex<dyn std::io::Write + Send + 'static>>),
}

impl Default for Target {
    fn default() -> Self {
        Target::Stdout
    }
}

impl Debug for Target {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Target::Stdout => write!(f, "Stdout"),
            Target::Stderr => write!(f, "Stderr"),
            Target::Writer(_) => write!(f, "Pipe"),
        }
    }
}
#[allow(dead_code)]
#[derive(Debug)]
struct WriterContext {
    name: String,
}

impl WriterContext {
    fn new(name: String) -> Self {
        Self { name }
    }
}

#[derive(Debug, Clone)]
pub struct StructuredWriter {
    target: Target,
    ctx: Arc<Mutex<VecDeque<WriterContext>>>,
}

impl StructuredWriter {
    pub fn new(target: Target) -> Self {
        Self {
            target,
            ctx: Arc::new(Mutex::new(VecDeque::new())),
        }
    }

    pub fn enter(&self, name: &str) -> WriterGuard<'_> {
        let wc = WriterContext::new(name.into());
        self.ctx.lock().unwrap().push_back(wc);
        WriterGuard::new(&self)
    }

    fn pop_level(&self) {
        let mut stack = self.ctx.lock().unwrap();
        if stack.len() > 0 {
            stack.pop_back();
        }
    }

    fn indent(&self) -> usize {
        let ctx = self.ctx.lock().unwrap();
        if ctx.len() > 0 {
            ctx.len() - 1
        } else {
            0
        }
    }

    pub fn write<S: AsRef<str>>(&self, msg: S) {
        return;
        let msg = msg.as_ref();
        let indent = self.indent();
        let indent_str = " ".repeat(2 * indent);
        if let Some(_ctx) = self.ctx.lock().unwrap().back_mut() {
            match &self.target {
                Target::Stdout => {
                    let stdout = std::io::stdout();
                    let mut handle = stdout.lock();
                    for line in msg.lines() {
                        writeln!(&mut handle, "{}{}", indent_str, line).unwrap();
                    }
                }
                Target::Stderr => todo!(),
                Target::Writer(_) => todo!(),
            }
        }
    }
}

pub struct WriterGuard<'writer> {
    writer: &'writer StructuredWriter,
}

impl<'writer> WriterGuard<'writer> {
    fn new(writer: &'writer StructuredWriter) -> Self {
        Self { writer }
    }
}

impl<'writer> Drop for WriterGuard<'writer> {
    fn drop(&mut self) {
        self.writer.pop_level();
    }
}
