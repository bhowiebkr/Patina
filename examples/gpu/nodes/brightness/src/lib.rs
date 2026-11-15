use patina_gpu::{GpuImage, gpu};
use patina_sdk::node;

/// Gives the mean of the red channel of the image, from 0 to 1. It copies the image to the CPU.
#[node]
pub fn brightness(image: &GpuImage) -> Result<f64, String> {
    let pixels = image.read(gpu()?)?;
    let sum: u64 = pixels.chunks_exact(4).map(|p| u64::from(p[0])).sum();
    Ok(sum as f64 / (pixels.len() / 4) as f64 / 255.0)
}
