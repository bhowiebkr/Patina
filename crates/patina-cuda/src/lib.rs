//! The shared CUDA context and the NVRTC compiler of node code (spec 10.1).
//!
//! All CUDA nodes of a process use the context of [`cuda`], also across clusters (10.2). A
//! node compiles CUDA C++ at run time with [`Cuda::kernel`] (10.3).

use std::sync::{Arc, OnceLock};

pub use cudarc;
use cudarc::driver::{CudaContext, CudaFunction, CudaStream};

/// The shared CUDA context of device 0 and its default stream.
pub struct Cuda {
    pub context: Arc<CudaContext>,
    pub stream: Arc<CudaStream>,
}

/// The context or the error of the first try. `patina_sdk::shared` keeps one for each process.
struct Shared(Result<Cuda, String>);

/// The shared CUDA context of the process. The error tells why Patina has no CUDA, for example
/// a computer with no NVIDIA driver.
pub fn cuda() -> Result<&'static Cuda, String> {
    static CUDA: OnceLock<&'static Shared> = OnceLock::new();
    CUDA.get_or_init(|| patina_sdk::shared(|| Shared(open()))).0.as_ref().map_err(Clone::clone)
}

fn open() -> Result<Cuda, String> {
    // SAFETY: the function only tries to load the driver library.
    if !unsafe { cudarc::driver::sys::is_culib_present() } {
        return Err("Patina found no NVIDIA CUDA driver".to_owned());
    }
    let context = CudaContext::new(0).map_err(|e| format!("Patina could not open CUDA device 0: {e}"))?;
    let stream = context.default_stream();
    Ok(Cuda { context, stream })
}

impl Cuda {
    /// Compiles CUDA C++ with NVRTC and gives the kernel `name`. The error has the compile log.
    pub fn kernel(&self, source: &str, name: &str) -> Result<CudaFunction, String> {
        // SAFETY: the function only tries to load the NVRTC library.
        if !unsafe { cudarc::nvrtc::sys::is_culib_present() } {
            return Err("Patina found no NVRTC library. Install the CUDA Toolkit 12".to_owned());
        }
        let ptx = cudarc::nvrtc::compile_ptx(source).map_err(|e| match e {
            cudarc::nvrtc::CompileError::CompileError { log, .. } => format!("{name}: {}", log.to_string_lossy()),
            other => format!("{name}: {other}"),
        })?;
        let module = self.context.load_module(ptx).map_err(|e| format!("{name}: {e}"))?;
        module.load_function(name).map_err(|e| format!("{name}: {e}"))
    }
}

#[cfg(test)]
mod tests;
