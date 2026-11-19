//! WGSL that reloads from disk while the app runs: edit a `.wgsl` file in the source tree and
//! the pipeline rebuilds within a quarter second, no Rust rebuild. Builds without the source
//! tree (moved exe) and packaged builds use the copy embedded at compile time.

use std::path::PathBuf;
use std::time::{Duration, SystemTime};
use web_time::Instant;

const POLL: Duration = Duration::from_millis(250);

/// A release package (`tools/package.py` sets `MW_BUILD_COMMIT`): it runs only the shaders and
/// kernels embedded at build time, never the source tree's newer files.
pub const PACKAGED: bool = option_env!("MW_BUILD_COMMIT").is_some();

pub struct ShaderFile {
    path: PathBuf,
    embedded: &'static str,
    live: bool,
    stamp: Option<SystemTime>,
    polled: Instant,
}

impl ShaderFile {
    /// `path` is absolute (use `concat!(env!("CARGO_MANIFEST_DIR"), "/...")`), `embedded` the
    /// same file through `include_str!`.
    pub fn new(path: &str, embedded: &'static str) -> Self {
        let path = PathBuf::from(path);
        let live = !PACKAGED;
        let stamp = live.then(|| Self::stamp(&path)).flatten();
        Self {
            path,
            embedded,
            live,
            stamp,
            polled: Instant::now(),
        }
    }

    fn stamp(path: &PathBuf) -> Option<SystemTime> {
        std::fs::metadata(path).and_then(|m| m.modified()).ok()
    }

    pub fn name(&self) -> String {
        self.path
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_default()
    }

    /// Current text: the file on disk when present, else the embedded copy.
    pub fn source(&self) -> String {
        if !self.live {
            return self.embedded.to_owned();
        }
        std::fs::read_to_string(&self.path).unwrap_or_else(|_| self.embedded.to_owned())
    }

    /// True once after the file on disk changes (checked at most every 250 ms).
    pub fn changed(&mut self) -> bool {
        if !self.live || self.polled.elapsed() < POLL {
            return false;
        }
        self.polled = Instant::now();
        let stamp = Self::stamp(&self.path);
        if stamp != self.stamp {
            self.stamp = stamp;
            return true;
        }
        false
    }
}

/// Compiles WGSL, returning the validation error instead of panicking the device.
pub fn compile(
    device: &wgpu::Device,
    label: &str,
    source: &str,
) -> Result<wgpu::ShaderModule, String> {
    validated(device, label, || {
        device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some(label),
            source: wgpu::ShaderSource::Wgsl(source.into()),
        })
    })
}

/// Runs `f` (pipeline or shader creation) and returns its validation error, if any, instead of
/// letting it reach the device's panicking error handler.
pub fn validated<T>(
    device: &wgpu::Device,
    label: &str,
    f: impl FnOnce() -> T,
) -> Result<T, String> {
    let internal = device.push_error_scope(wgpu::ErrorFilter::Internal);
    let memory = device.push_error_scope(wgpu::ErrorFilter::OutOfMemory);
    let scope = device.push_error_scope(wgpu::ErrorFilter::Validation);
    let value = f();
    let validation_error = finish_scope(scope, label);
    let memory_error = finish_scope(memory, label);
    let internal_error = finish_scope(internal, label);
    if let Some(error) = validation_error.or(memory_error).or(internal_error) {
        Err(error)
    } else {
        Ok(value)
    }
}

pub fn finish_scope(scope: wgpu::ErrorScopeGuard, label: &str) -> Option<String> {
    #[cfg(not(target_arch = "wasm32"))]
    {
        pollster::block_on(scope.pop()).map(|e| format!("{label}: {e}"))
    }
    #[cfg(target_arch = "wasm32")]
    {
        let label = label.to_owned();
        let future = scope.pop();
        wasm_bindgen_futures::spawn_local(async move {
            if let Some(error) = future.await {
                web_sys::console::error_1(&format!("{label}: {error}").into());
            }
        });
        None
    }
}
