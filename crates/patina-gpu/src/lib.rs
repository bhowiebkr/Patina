//! The shared GPU context, GPU types and the WESL linker of node code (spec 10).
//!
//! All GPU nodes of a process use the device of [`gpu`], also across clusters (10.2). A node
//! links its WESL files with [`link`] and compiles the WGSL with [`Gpu::shader`] (10.3).

mod image;
#[cfg(feature = "wesl")]
mod link;
mod viewport;

use std::future::Future;
use std::pin::pin;
use std::sync::{Arc, OnceLock};
use std::task::{Context, Poll, Waker};

pub use image::GpuImage;
#[cfg(feature = "wesl")]
pub use link::{PACKAGE, link, link_source};
pub use viewport::ViewportSurface;
pub use wgpu;

/// The shared GPU context.
pub struct Gpu {
    pub instance: wgpu::Instance,
    pub adapter: wgpu::Adapter,
    pub device: wgpu::Device,
    pub queue: wgpu::Queue,
}

/// The context or the error of the first try. `patina_sdk::shared` keeps one for each process.
struct Shared(Result<Gpu, String>);

/// The shared GPU context of the process. It uses the software adapter of the system when the
/// computer has no GPU. The error tells why Patina found no adapter.
pub fn gpu() -> Result<&'static Gpu, String> {
    static GPU: OnceLock<&'static Shared> = OnceLock::new();
    GPU.get_or_init(|| patina_sdk::shared(|| Shared(open()))).0.as_ref().map_err(Clone::clone)
}

fn open() -> Result<Gpu, String> {
    let mut desc = wgpu::InstanceDescriptor::new_without_display_handle();
    // The viewport surface under the web view needs DX12 on Windows (15.8).
    if cfg!(windows) {
        desc.backends = wgpu::Backends::DX12;
    }
    let instance = wgpu::Instance::new(desc.with_env());
    let fast = wgpu::RequestAdapterOptions { power_preference: wgpu::PowerPreference::HighPerformance, ..Default::default() };
    let soft = wgpu::RequestAdapterOptions { force_fallback_adapter: true, ..Default::default() };
    let adapter = block_on(instance.request_adapter(&fast))
        .or_else(|_| block_on(instance.request_adapter(&soft)))
        .map_err(|e| format!("Patina found no GPU adapter: {e}"))?;
    let (device, queue) = block_on(adapter.request_device(&wgpu::DeviceDescriptor::default())).map_err(|e| e.to_string())?;
    // The default handler stops the process. Each call that can fail uses an error scope.
    device.on_uncaptured_error(Arc::new(|error| eprintln!("Patina: GPU error: {error}")));
    Ok(Gpu { instance, adapter, device, queue })
}

impl Gpu {
    /// Compiles WGSL, for example the text of [`link`]. The error has the line of the WGSL.
    pub fn shader(&self, label: &str, wgsl: &str) -> Result<wgpu::ShaderModule, String> {
        let scope = self.device.push_error_scope(wgpu::ErrorFilter::Validation);
        let desc = wgpu::ShaderModuleDescriptor { label: Some(label), source: wgpu::ShaderSource::Wgsl(wgsl.into()) };
        let module = self.device.create_shader_module(desc);
        match block_on(scope.pop()) {
            Some(error) => Err(format!("{label}: {error}")),
            None => Ok(module),
        }
    }
}

/// Runs a future of wgpu to its end. The native futures of wgpu are ready at the first poll.
pub fn block_on<F: Future>(future: F) -> F::Output {
    let mut future = pin!(future);
    let mut cx = Context::from_waker(Waker::noop());
    loop {
        if let Poll::Ready(value) = future.as_mut().poll(&mut cx) {
            return value;
        }
        std::thread::yield_now();
    }
}

#[cfg(test)]
mod tests;
