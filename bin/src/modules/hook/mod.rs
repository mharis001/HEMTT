use std::sync::{Arc, Mutex};

use ::rhai::{packages::Package, Engine, Scope};
use hemtt_common::workspace::WorkspacePath;

use crate::{context::Context, error::Error, report::Report};

use self::{
    error::{
        bhe1_script_not_found::ScriptNotFound, bhe2_script_fatal::ScriptFatal,
        bhe3_parse_error::RhaiParseError, bhe4_runtime_error::RuntimeError,
    },
    libraries::hemtt::RhaiHemtt,
};

use super::Module;

mod error;
mod libraries;
mod time;

/// Creates a sccope for a Rhai script
///
/// # Errors
/// [`Error::Version`] if the version is not a valid semver version
pub fn scope(ctx: &Context, vfs: bool) -> Result<Scope, Error> {
    let mut scope = Scope::new();
    if vfs {
        scope.push_constant("HEMTT_VFS", ctx.workspace().vfs().clone());
    }
    scope.push_constant("HEMTT_DIRECTORY", ctx.project_folder().clone());
    scope.push_constant("HEMTT_OUTPUT", ctx.build_folder().clone());
    scope.push_constant("HEMTT_RFS", ctx.project_folder().clone());
    scope.push_constant("HEMTT_OUT", ctx.build_folder().clone());

    scope.push_constant("HEMTT", RhaiHemtt::new(ctx));

    Ok(scope)
}

fn engine(vfs: bool) -> Engine {
    let mut engine = Engine::new();
    if vfs {
        let virt = libraries::VfsPackage::new();
        engine.register_static_module("hemtt_vfs", virt.as_shared_module());
    }
    engine.register_static_module("hemtt_rfs", libraries::RfsPackage::new().as_shared_module());
    engine.register_static_module("hemtt", libraries::HEMTTPackage::new().as_shared_module());
    engine.register_fn("date", time::date);
    engine
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Hooks(bool);

impl Default for Hooks {
    fn default() -> Self {
        Self(true)
    }
}

impl Hooks {
    /// Run a folder of hooks
    ///
    /// # Errors
    /// [`Error::ScriptNotFound`] if the script does not exist
    /// [`Error::HookFatal`] if the script calls `fatal`
    /// [`Error::Rhai`] if the script is invalid
    ///
    /// # Panics
    /// If a file path is not a valid [`OsStr`] (UTF-8)
    pub fn run_folder(self, ctx: &Context, name: &str, vfs: bool) -> Result<(), Error> {
        if !self.0 {
            return Ok(());
        }
        let folder = ctx.workspace().join(".hemtt")?.join("hooks")?.join(name)?;
        if !folder.exists()? {
            trace!("no {} hooks", name);
            return Ok(());
        }
        let mut entries = folder.read_dir()?;
        entries.sort_by_key(WorkspacePath::filename);
        for file in entries {
            if !file.is_file()? {
                continue;
            }
            info!(
                "Running hook: {}",
                file.as_str().trim_start_matches("/.hemtt/hooks/")
            );
            Self::run(ctx, file, vfs)?;
            ctx.config().version().invalidate();
        }
        Ok(())
    }

    /// Run a script
    ///
    /// # Errors
    /// [`Error::ScriptNotFound`] if the script does not exist
    /// [`Error::HookFatal`] if the script calls `fatal`
    /// [`Error::Rhai`] if the script is invalid
    ///
    /// # Panics
    /// If a file path is not a valid [`OsStr`] (UTF-8)
    pub fn run_file(ctx: &Context, name: &str) -> Result<Report, Error> {
        let mut report = Report::new();
        let scripts = ctx.workspace().join(".hemtt")?.join("scripts")?;
        let path = scripts.join(name)?.with_extension("rhai")?;
        trace!("running script: {}", path.as_str());
        if !path.exists()? {
            report.error(ScriptNotFound::code(name.to_owned(), &scripts)?);
            return Ok(report);
        }
        let res = Self::run(ctx, path, false);
        ctx.config().version().invalidate();
        res
    }

    #[allow(clippy::needless_pass_by_value)] // rhai things
    fn run(ctx: &Context, path: WorkspacePath, vfs: bool) -> Result<Report, Error> {
        let mut report = Report::new();
        let mut engine = engine(vfs);
        let mut scope = scope(ctx, vfs)?;
        let told_to_fail = Arc::new(Mutex::new(false));
        let parts = path.as_str().split('/');
        let name = parts
            .clone()
            .skip(parts.count().saturating_sub(2))
            .collect::<Vec<_>>()
            .join("/");
        let inner_name = name.clone();
        engine.on_debug(move |x, _src, _pos| {
            debug!("[{inner_name}] {x}");
        });
        let inner_name = name.clone();
        engine.on_print(move |s| {
            info!("[{inner_name}] {s}");
        });
        let inner_name = name.clone();
        engine.register_fn("info", move |s: &str| {
            info!("[{inner_name}] {s}");
        });
        let inner_name = name.clone();
        engine.register_fn("warn", move |s: &str| {
            warn!("[{inner_name}] {s}");
        });
        let inner_name = name.clone();
        engine.register_fn("error", move |s: &str| {
            error!("[{inner_name}] {s}");
        });
        let inner_name = name.clone();
        let inner_told_to_fail = told_to_fail.clone();
        engine.register_fn("fatal", move |s: &str| {
            error!("[{inner_name}] {s}");
            *inner_told_to_fail.lock().unwrap() = true;
        });
        if let Err(e) = engine.run_with_scope(&mut scope, &path.read_to_string()?) {
            report.error(RuntimeError::code(path, &e));
            return Ok(report);
        }
        if *told_to_fail.lock().unwrap() {
            report.error(ScriptFatal::code(name));
        }
        Ok(report)
    }
}

impl Module for Hooks {
    fn name(&self) -> &'static str {
        "Hooks"
    }

    fn init(&mut self, ctx: &Context) -> Result<Report, Error> {
        let mut report = Report::new();
        self.0 = ctx.hemtt_folder().join("hooks").exists();
        if self.0 {
            for phase in &["pre_build", "post_build", "pre_release", "post_release"] {
                let engine = engine(phase != &"post_release");
                let dir = ctx.hemtt_folder().join("hooks").join(phase);
                if !dir.exists() {
                    continue;
                }
                for hook in dir.read_dir().unwrap() {
                    let hook = hook?;
                    let path = ctx
                        .workspace()
                        .join(".hemtt")?
                        .join("hooks")?
                        .join(phase)?
                        .join(hook.file_name().to_str().unwrap())?;
                    if let Err(e) = engine.compile(&path.read_to_string()?) {
                        report.error(RhaiParseError::code(path, e.0, e.1));
                    }
                }
            }
        } else {
            trace!("no hooks folder");
        }
        Ok(report)
    }

    fn pre_build(&self, ctx: &Context) -> Result<Report, Error> {
        self.run_folder(ctx, "pre_build", true)?;
        Ok(Report::new())
    }

    fn post_build(&self, ctx: &Context) -> Result<Report, Error> {
        self.run_folder(ctx, "post_build", true)?;
        Ok(Report::new())
    }

    fn pre_release(&self, ctx: &Context) -> Result<Report, Error> {
        self.run_folder(ctx, "pre_release", true)?;
        Ok(Report::new())
    }

    fn post_release(&self, ctx: &Context) -> Result<Report, Error> {
        self.run_folder(ctx, "post_release", false)?;
        Ok(Report::new())
    }
}
