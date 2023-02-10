use std::str::FromStr;

use crate::error::TaskError;
use mlua::UserData;
use serde::Deserialize;
use zbus::zvariant::{ObjectPath, OwnedObjectPath, Type};
use zbus::{blocking::Connection, dbus_proxy};

type Result<T, E = TaskError> = core::result::Result<T, E>;

#[derive(Debug, Type, Deserialize)]
pub struct UnitChangeInfo {
    pub ty: String,
    pub file: String,
    pub dest: String,
}

#[derive(Debug, Type, Deserialize)]
pub struct UnitChange {
    pub has_install_info: bool,
    pub changes: Vec<UnitChangeInfo>,
}

#[dbus_proxy(
    interface = "org.freedesktop.systemd1.Manager",
    default_service = "org.freedesktop.systemd1",
    default_path = "/org/freedesktop/systemd1"
)]
trait Systemd {
    /// Get path DBus object path to the Unit.  
    /// All of the below methods that take a unit name would also be available on that object.
    fn get_unit(&self, name: &str) -> zbus::Result<OwnedObjectPath>;

    /// Tell systemd to reload unit files
    fn reload(&self) -> zbus::Result<()>;

    /// Start a unit by name.  
    /// `mode` is one of "replace", "fail", "isolate", "ignore-dependencies", or "ignore-requirements".     
    fn start_unit(&self, name: &str, mode: &str) -> zbus::Result<OwnedObjectPath>;

    /// Similar to `start_unit`, but for restarts.  If the requested unit is stopped, it is started.
    fn restart_unit(&self, name: &str, mode: &str) -> zbus::Result<OwnedObjectPath>;

    /// Similar to `start_unit`, but for stops.
    fn stop_unit(&self, name: &str, mode: &str) -> zbus::Result<OwnedObjectPath>;

    /// Similar to `start_unit`, but for reloads.
    fn reload_unit(&self, name: &str, mode: &str) -> zbus::Result<OwnedObjectPath>;

    /// Similar to `start_unit`, but tries to reload if supported, falling back to restarting.
    fn reload_or_restart_unit(&self, name: &str, mode: &str) -> zbus::Result<OwnedObjectPath>;

    /// Call before a job method to ensure that you'll be notified of job updates when you listen to signals
    fn subscribe(&self) -> zbus::Result<()>;

    /// Call when done with receiving signals.  Not actually needed as systemd tracks client connections.
    fn unsubscribe(&self) -> zbus::Result<()>;

    /// Enable the given list of unit files.
    fn enable_unit_files(
        &self,
        units: &[&str],
        runtime: bool,
        force: bool,
    ) -> zbus::Result<UnitChange>;

    /// Disable the given list of unit files.
    fn disable_unit_files(
        &self,
        units: &[&str],
        runtime: bool,
        force: bool,
    ) -> zbus::Result<UnitChange>;

    /// Mask the given list of unit files.
    fn mask_unit_files(
        &self,
        units: &[&str],
        runtime: bool,
        force: bool,
    ) -> zbus::Result<Vec<UnitChangeInfo>>;

    /// Unmask the given list of unit files
    fn unmask_unit_files(&self, units: &[&str], runtime: bool)
        -> zbus::Result<Vec<UnitChangeInfo>>;

    /// Called when a job is dequeued (when it finishes).
    /// `result` is one of "done", "canceled", "timeout", "failed", "dependency", or "skipped"
    #[dbus_proxy(signal)]
    fn job_removed(
        &self,
        id: u32,
        job: ObjectPath<'_>,
        unit: &str,
        result: &str,
    ) -> zbus::Result<()>;

    #[dbus_proxy(signal)]
    fn unit_new(&self, id: u32, unit: ObjectPath<'_>) -> zbus::Result<()>;

    #[dbus_proxy(signal)]
    fn unit_removed(&self, id: u32, unit: ObjectPath<'_>) -> zbus::Result<()>;
}

#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub enum JobResult {
    Done,
    Canceled,
    Timeout,
    Failed,
    Dependency,
    Skipped,
}

impl JobResult {
    pub fn to_lua(&self) -> &'static str {
        match self {
            JobResult::Done => "done",
            JobResult::Canceled => "canceled",
            JobResult::Timeout => "timeout",
            JobResult::Failed => "failed",
            JobResult::Dependency => "dependency",
            JobResult::Skipped => "skipped",
        }
    }
}

impl UserData for JobResult {
    fn add_methods<'lua, T: mlua::UserDataMethods<'lua, Self>>(methods: &mut T) {
        methods.add_method("failed", |_, &this, _: ()| Ok(this != JobResult::Done));

        methods.add_method("successful", |_, &this, _: ()| Ok(this == JobResult::Done));

        methods.add_method("result", |_, &this, _: ()| Ok(this.to_lua()));
    }
}

impl FromStr for JobResult {
    type Err = TaskError;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        use JobResult::*;
        match s.to_ascii_lowercase().as_str() {
            "done" => Ok(Done),
            "canceled" => Ok(Canceled),
            "timeout" => Ok(Timeout),
            "failed" => Ok(Failed),
            "dependency" => Ok(Dependency),
            "skipped" => Ok(Skipped),
            t @ _ => Err(TaskError::ActionError(format!("Invalid job result {}", t))),
        }
    }
}

#[derive(Debug, Clone)]
pub struct SystemdUnit {
    unit: String,
    conn: Connection,
}

impl SystemdUnit {
    pub fn system<S: Into<String>>(unit: S) -> Result<SystemdUnit> {
        let conn = Connection::system()?;
        let manager = SystemdProxyBlocking::new(&conn)?;
        manager.subscribe()?;
        Ok(SystemdUnit {
            unit: unit.into(),
            conn,
        })
    }

    pub fn session<S: Into<String>>(unit: S) -> Result<SystemdUnit> {
        let conn = Connection::session()?;
        let manager = SystemdProxyBlocking::new(&conn)?;
        manager.subscribe()?;
        Ok(SystemdUnit {
            unit: unit.into(),
            conn,
        })
    }

    fn manager(&self) -> Result<SystemdProxyBlocking> {
        Ok(SystemdProxyBlocking::new(&self.conn)?)
    }

    fn wait_for_job(&self, job: &ObjectPath) -> Result<JobResult> {
        // loop through received signals, looking for the job reference from our unit state change
        // keep looping until we receive some sort of notification about the job
        loop {
            for signal in self.manager()?.receive_job_removed()? {
                let args = signal.args()?;
                if args.job() == job {
                    return Ok(JobResult::from_str(args.result())?);
                }
            }
        }
    }

    pub fn daemon_reload(&self) -> Result<()> {
        Ok(self.manager()?.reload()?)
    }

    pub fn reload(&self) -> Result<JobResult> {
        let job = self.manager()?.reload_unit(&self.unit, "replace")?;
        Ok(self.wait_for_job(&job)?)
    }

    pub fn restart(&self) -> Result<JobResult> {
        let job = self.manager()?.restart_unit(&self.unit, "replace")?;
        Ok(self.wait_for_job(&job)?)
    }

    pub fn reload_or_restart(&self) -> Result<JobResult> {
        let job = self
            .manager()?
            .reload_or_restart_unit(&self.unit, "replace")?;
        Ok(self.wait_for_job(&job)?)
    }
    pub fn start(&self) -> Result<JobResult> {
        let job = self.manager()?.start_unit(&self.unit, "replace")?;
        Ok(self.wait_for_job(&job)?)
    }

    pub fn stop(&self) -> Result<JobResult> {
        let job = self.manager()?.stop_unit(&self.unit, "replace")?;
        Ok(self.wait_for_job(&job)?)
    }

    pub fn enable(&self, force: bool) -> Result<UnitChange> {
        Ok(self
            .manager()?
            .enable_unit_files(&[&self.unit], false, force)?)
    }

    pub fn disable(&self, force: bool) -> Result<UnitChange> {
        Ok(self
            .manager()?
            .disable_unit_files(&[&self.unit], false, force)?)
    }

    pub fn mask(&self, force: bool) -> Result<Vec<UnitChangeInfo>> {
        Ok(self
            .manager()?
            .mask_unit_files(&[&self.unit], false, force)?)
    }

    pub fn unmask(&self) -> Result<Vec<UnitChangeInfo>> {
        Ok(self.manager()?.unmask_unit_files(&[&self.unit], false)?)
    }

    pub fn service(&self) -> &str {
        &self.unit
    }
}
