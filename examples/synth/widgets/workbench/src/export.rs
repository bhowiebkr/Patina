use patina_sdk::{Graph, PathKind, Ui, project_path};
use std::{
    fs::{self, File},
    io::{Read, Seek, SeekFrom},
    path::{Path, PathBuf},
    process::{Child, Command, Stdio},
    time::{SystemTime, UNIX_EPOCH},
};

pub struct Export {
    path: String,
    renderer: String,
    seconds: f32,
    job: Option<Job>,
    status: String,
    setup: bool,
}
struct Job {
    child: Child,
    folder: PathBuf,
    target: PathBuf,
    samples: u64,
}
impl Default for Export {
    fn default() -> Self {
        let renderer = std::env::current_exe()
            .ok()
            .and_then(|p| {
                p.parent().map(|p| {
                    p.join(if cfg!(windows) {
                        "patina-cli.exe"
                    } else {
                        "patina-cli"
                    })
                })
            })
            .unwrap_or_default();
        Self {
            path: "ember.wav".into(),
            setup: !renderer.is_file(),
            renderer: renderer.to_string_lossy().into(),
            seconds: 10.0,
            job: None,
            status: String::new(),
        }
    }
}
impl Export {
    pub fn ui(&mut self, ui: &mut Ui, _: &mut Graph) {
        if let Some(job) = &mut self.job {
            match job.child.try_wait() {
                Ok(Some(status)) => {
                    let result = if status.success() {
                        job.publish()
                    } else {
                        Err(format!("Render failed: {}", job.log()))
                    };
                    self.status = match result {
                        Ok(()) => format!("Wrote {}", job.target.display()),
                        Err(e) => e,
                    };
                    if let Err(e) = job.cleanup() {
                        self.status.push_str(&format!("; {e}"));
                    }
                    self.job = None;
                }
                Err(e) => self.status = format!("Cannot read render status: {e}"),
                _ => {}
            }
        }
        ui.columns(240.0, |ui| {
            ui.column(|ui| {
                ui.file_path("WAV destination", &mut self.path, PathKind::Save);
            });
            ui.column(|ui| {
                ui.number_unit("Duration", &mut self.seconds, 1.0, "seconds");
            });
        });
        ui.label("Saved main graph / mono / 48000 Hz / 32-bit float WAV");
        ui.label("Export reads saved files. Muted saved output renders silence. Enable Device output in Audio, then Save.");
        if let Some(job) = &self.job {
            let written = fs::metadata(job.folder.join("audio.wav"))
                .map(|m| m.len().saturating_sub(58))
                .unwrap_or(0);
            ui.progress(
                if written == 0 {
                    "Building saved patch"
                } else {
                    "Rendering audio"
                },
                written as f32 / (job.samples * 4) as f32,
            );
            if ui.button("Cancel export").clicked() {
                self.cancel();
            }
        } else if ui.button("Export saved patch").clicked() {
            self.status = match self.start() {
                Ok(job) => {
                    self.job = Some(job);
                    "Render started".into()
                }
                Err(e) => e,
            };
        }
        ui.section("Renderer executable", &mut self.setup, |ui| {
            ui.file_path("Patina CLI", &mut self.renderer, PathKind::Open);
        });
        if !self.status.is_empty() {
            ui.label(&self.status);
        }
    }
    fn start(&self) -> Result<Job, String> {
        if !self.seconds.is_finite() || !(0.01..=3600.0).contains(&self.seconds) {
            return Err("Use a duration from 0.01 to 3600 seconds".into());
        }
        let target = project_path(&self.path);
        if target
            .extension()
            .is_none_or(|e| !e.eq_ignore_ascii_case("wav"))
        {
            return Err("Choose a .wav destination".into());
        }
        if target.try_exists().map_err(|e| e.to_string())? {
            return Err("The destination exists. Choose a new name.".into());
        }
        let parent = target.parent().ok_or("Choose a destination folder")?;
        if !parent.is_dir() {
            return Err("The destination folder does not exist".into());
        }
        let folder = parent.join(format!(
            ".ember-render-{}-{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_nanos()
        ));
        fs::create_dir(&folder).map_err(|e| e.to_string())?;
        let spawn = (|| {
            let log = File::create_new(folder.join("render.log")).map_err(|e| e.to_string())?;
            let mut command = Command::new(&self.renderer);
            command
                .arg("render")
                .arg(project_path(""))
                .arg(folder.join("audio.wav"))
                .args([
                    "--seconds",
                    &self.seconds.to_string(),
                    "--graph",
                    "main",
                    "--edit",
                ])
                .stdin(Stdio::null())
                .stdout(Stdio::null())
                .stderr(log);
            hidden(&mut command);
            command
                .spawn()
                .map_err(|e| format!("Cannot start Patina CLI: {e}"))
        })();
        match spawn {
            Ok(child) => Ok(Job {
                child,
                folder,
                target,
                samples: (f64::from(self.seconds) * 48000.0).round() as u64,
            }),
            Err(error) => {
                let _ = fs::remove_file(folder.join("render.log"));
                let cleanup = fs::remove_dir(&folder);
                Err(format!("{error}; temporary folder cleanup: {cleanup:?}"))
            }
        }
    }
    fn cancel(&mut self) {
        if let Some(mut job) = self.job.take() {
            match job.stop() {
                Ok(()) => {
                    self.status = job.cleanup().map_or_else(
                        |e| e,
                        |_| "Export cancelled. No destination file was written.".into(),
                    )
                }
                Err(e) => {
                    self.status = e;
                    self.job = Some(job);
                }
            }
        }
    }
}
impl Drop for Export {
    fn drop(&mut self) {
        self.cancel();
    }
}
impl Job {
    fn publish(&self) -> Result<(), String> {
        let source = self.folder.join("audio.wav");
        let length = fs::metadata(&source).map_err(|e| e.to_string())?.len();
        if length != 58 + self.samples * 4 {
            return Err("The renderer returned an incomplete WAV".into());
        }
        // A hard link publishes the complete file without replacing an existing destination.
        if fs::hard_link(&source, &self.target).is_ok() {
            return Ok(());
        }
        copy_new(&source, &self.target)
            .map_err(|e| format!("Cannot publish {}: {e}", self.target.display()))
    }
    fn log(&self) -> String {
        let mut text = String::new();
        if let Ok(mut file) = File::open(self.folder.join("render.log")) {
            let length = file.metadata().map(|m| m.len()).unwrap_or(0);
            let _ = file.seek(SeekFrom::Start(length.saturating_sub(4000)));
            let _ = file.read_to_string(&mut text);
        }
        text
    }
    fn cleanup(&self) -> Result<(), String> {
        for name in ["audio.wav", "render.log"] {
            let path = self.folder.join(name);
            if path.exists() {
                fs::remove_file(&path)
                    .map_err(|e| format!("Cannot remove {}: {e}", path.display()))?;
            }
        }
        fs::remove_dir(&self.folder)
            .map_err(|e| format!("Cannot remove {}: {e}", self.folder.display()))
    }
    fn stop(&mut self) -> Result<(), String> {
        if self.child.try_wait().map_err(|e| e.to_string())?.is_none() {
            #[cfg(windows)]
            {
                let mut command = Command::new("taskkill.exe");
                command.args(["/PID", &self.child.id().to_string(), "/T", "/F"]);
                hidden(&mut command);
                if !command
                    .stdout(Stdio::null())
                    .stderr(Stdio::null())
                    .status()
                    .map_err(|e| e.to_string())?
                    .success()
                {
                    return Err("Cannot stop the render process tree".into());
                }
            }
            #[cfg(unix)]
            {
                if !Command::new("kill")
                    .args(["-TERM", "--", &format!("-{}", self.child.id())])
                    .status()
                    .map_err(|e| e.to_string())?
                    .success()
                {
                    return Err("Cannot stop the render process group".into());
                }
            }
            self.child.wait().map_err(|e| e.to_string())?;
        }
        Ok(())
    }
}

// FAT volumes cannot create hard links. A new file still protects existing exports.
fn copy_new(source: &Path, target: &Path) -> Result<(), String> {
    let mut input = File::open(source).map_err(|e| e.to_string())?;
    let mut output = File::create_new(target).map_err(|e| e.to_string())?;
    let result = std::io::copy(&mut input, &mut output).and_then(|_| output.sync_all());
    drop(output);
    if let Err(error) = result {
        let cleanup = fs::remove_file(target);
        return Err(format!("{error}; partial file cleanup: {cleanup:?}"));
    }
    Ok(())
}

fn hidden(command: &mut Command) {
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        command.creation_flags(0x08000000);
    }
    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt;
        command.process_group(0);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn fallback_copy_preserves_existing_destinations() {
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let folder =
            std::env::temp_dir().join(format!("ember-export-{}-{stamp}", std::process::id()));
        fs::create_dir(&folder).unwrap();
        let source = folder.join("source.wav");
        let target = folder.join("target.wav");
        fs::write(&source, b"sound").unwrap();
        copy_new(&source, &target).unwrap();
        fs::write(&source, b"changed").unwrap();
        assert!(copy_new(&source, &target).is_err());
        assert_eq!(fs::read(&target).unwrap(), b"sound");
        fs::remove_file(source).unwrap();
        fs::remove_file(target).unwrap();
        fs::remove_dir(folder).unwrap();
    }
}
