use rlua::{Lua, UserData};

use crate::actions::util;
use crate::error::TaskError;
use crate::{Result, WRITER};

use self::systemd::{JobResult, SystemdUnit};
pub mod systemd;

pub struct HpgSystemdUnit {
    unit: SystemdUnit,
}

impl UserData for HpgSystemdUnit {
    fn add_methods<'lua, T: rlua::UserDataMethods<'lua, Self>>(methods: &mut T) {
        // Service Control Methods
        methods.add_method("start", |_, this, _: ()| {
            WRITER.write(format!("Starting service {}...", this.unit.service()));
            let res = this.unit.start().map_err(util::task_error)?;
            WRITER.write(format!(
                "Starting service {}: {}",
                this.unit.service(),
                res.to_lua()
            ));
            Ok(res)
        });

        methods.add_method("must_start", |_, this, _: ()| {
            WRITER.write(format!("Starting service {}...", this.unit.service()));
            let res = this.unit.start().map_err(util::task_error)?;
            if res != JobResult::Done {
                return Err(util::action_error(format!(
                    "Service {} failed to start",
                    this.unit.service()
                )));
            }
            WRITER.write(format!(
                "Starting service {}: {}",
                this.unit.service(),
                res.to_lua()
            ));
            Ok(())
        });

        methods.add_method("stop", |_, this, _: ()| {
            WRITER.write(format!("Stopping service {}...", this.unit.service()));
            let res = this.unit.stop().map_err(util::task_error)?;
            WRITER.write(format!(
                "Stopping service {}: {}",
                this.unit.service(),
                res.to_lua()
            ));
            Ok(res)
        });

        methods.add_method("must_stop", |_, this, _: ()| {
            WRITER.write(format!("Stopping service {}...", this.unit.service()));
            let res = this.unit.start().map_err(util::task_error)?;
            if res != JobResult::Done {
                return Err(util::action_error(format!(
                    "Service {} failed to stop",
                    this.unit.service()
                )));
            }
            WRITER.write(format!(
                "Stopping service {}: {}",
                this.unit.service(),
                res.to_lua()
            ));
            Ok(())
        });

        methods.add_method("reload", |_, this, _: ()| {
            WRITER.write(format!("Reloading service {}...", this.unit.service()));
            let res = this.unit.reload().map_err(util::task_error)?;
            WRITER.write(format!(
                "Reloading service {}: {}",
                this.unit.service(),
                res.to_lua()
            ));
            Ok(res)
        });

        methods.add_method("must_reload", |_, this, _: ()| {
            WRITER.write(format!("Reloading service {}...", this.unit.service()));
            let res = this.unit.reload().map_err(util::task_error)?;
            if res != JobResult::Done {
                return Err(util::action_error(format!(
                    "Service {} failed to reload",
                    this.unit.service()
                )));
            }
            WRITER.write(format!(
                "Reloading service {}: {}",
                this.unit.service(),
                res.to_lua()
            ));
            Ok(())
        });

        methods.add_method("restart", |_, this, _: ()| {
            WRITER.write(format!("Restarting service {}...", this.unit.service()));
            let res = this.unit.restart().map_err(util::task_error)?;
            WRITER.write(format!(
                "Restarting service {}: {}",
                this.unit.service(),
                res.to_lua()
            ));
            Ok(res)
        });

        methods.add_method("must_restart", |_, this, _: ()| {
            WRITER.write(format!("Restarting service {}...", this.unit.service()));
            let res = this.unit.restart().map_err(util::task_error)?;
            if res != JobResult::Done {
                return Err(util::action_error(format!(
                    "Service {} failed to restart",
                    this.unit.service()
                )));
            }
            WRITER.write(format!(
                "Restarting service {}: {}",
                this.unit.service(),
                res.to_lua()
            ));
            Ok(())
        });

        methods.add_method("reload_or_restart", |_, this, _: ()| {
            WRITER.write(format!("Reload/restart service {}...", this.unit.service()));
            let res = this.unit.reload_or_restart().map_err(util::task_error)?;
            WRITER.write(format!(
                "Reload/restart service {}: {}",
                this.unit.service(),
                res.to_lua()
            ));
            Ok(res)
        });

        methods.add_method("must_reload_or_restart", |_, this, _: ()| {
            WRITER.write(format!("Reload/restart service {}...", this.unit.service()));
            let res = this.unit.reload_or_restart().map_err(util::task_error)?;
            if res != JobResult::Done {
                return Err(util::action_error(format!(
                    "Service {} failed to reload or restart",
                    this.unit.service()
                )));
            }
            WRITER.write(format!(
                "Reload/restart service {}: {}",
                this.unit.service(),
                res.to_lua()
            ));
            Ok(())
        });

        // Service activation methods
        methods.add_method("enable", |_, this, _: ()| {
            WRITER.write(format!("Enable service {}", this.unit.service()));
            this.unit.enable(false).map_err(util::task_error)?;
            Ok(())
        });

        methods.add_method("force_enable", |_, this, _: ()| {
            WRITER.write(format!("Enable service {} (forced)", this.unit.service()));
            this.unit.enable(true).map_err(util::task_error)?;
            Ok(())
        });

        methods.add_method("disable", |_, this, _: ()| {
            WRITER.write(format!("Disable service {}", this.unit.service()));
            this.unit.disable(false).map_err(util::task_error)?;
            Ok(())
        });

        methods.add_method("force_disable", |_, this, _: ()| {
            WRITER.write(format!("Disable service {} (forced)", this.unit.service()));
            this.unit.disable(true).map_err(util::task_error)?;
            Ok(())
        });

        methods.add_method("mask", |_, this, _: ()| {
            WRITER.write(format!("Mask service {}", this.unit.service()));
            this.unit.mask(false).map_err(util::task_error)?;
            Ok(())
        });

        methods.add_method("force_mask", |_, this, _: ()| {
            WRITER.write(format!("Mask service {} (forced)", this.unit.service()));
            this.unit.mask(true).map_err(util::task_error)?;
            Ok(())
        });

        methods.add_method("unmask", |_, this, _: ()| {
            WRITER.write(format!("Unmask service {}", this.unit.service()));
            this.unit.unmask().map_err(util::task_error)?;
            Ok(())
        });
    }
}

pub fn systemd_service(lua: &Lua) -> Result<()> {
    lua.context::<_, Result<(), TaskError>>(|lua_ctx| {
        let mod_systemd = lua_ctx.create_table()?;

        let system = lua_ctx.create_function(|_, unit: String| {
            let unit = SystemdUnit::system(unit).map_err(util::task_error)?;
            Ok(HpgSystemdUnit { unit })
        })?;
        mod_systemd.set("system", system)?;

        let session = lua_ctx.create_function(|_, unit: String| {
            let unit = SystemdUnit::session(unit).map_err(util::task_error)?;
            Ok(HpgSystemdUnit { unit })
        })?;
        mod_systemd.set("session", session)?;
        lua_ctx.globals().set("systemd", mod_systemd)?;
        Ok(())
    })?;
    Ok(())
}
