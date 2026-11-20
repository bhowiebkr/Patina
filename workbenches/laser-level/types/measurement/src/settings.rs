use crate::core::{beam::FinderConfig, camera::CameraSettings, reading::SampleRule};
use serde::{Serialize, de::DeserializeOwned};

pub fn decode<T: DeserializeOwned + Default>(text: &str) -> Result<T, String> {
    if text.is_empty() {
        return Ok(T::default());
    }
    if text.len() > 1_048_576 {
        return Err("Settings exceed 1 MB".into());
    }
    serde_json::from_str(text).map_err(|e| e.to_string())
}
pub fn encode<T: Serialize>(value: &T) -> String {
    serde_json::to_string(value).expect("validated settings")
}

pub fn finder(text: &str) -> Result<FinderConfig, String> {
    let c: FinderConfig = decode(text)?;
    if !(3..=128).contains(&c.strips)
        || !range(c.min_snr, 1.0, 1000.0)
        || !range(c.window_sigmas, 0.5, 10.0)
        || !range(c.background_sigmas, 1.0, 20.0)
        || !range(c.max_clipped, 0.0, 1.0)
        || !range(c.max_tilt, 0.0, 1.0)
        || !range(c.max_width, 0.01, 1.0)
    {
        return Err("Invalid beam finder settings".into());
    }
    Ok(c)
}
pub fn camera(text: &str) -> Result<CameraSettings, String> {
    let c: CameraSettings = decode(text)?;
    if c.mode.is_some_and(|m| m.width == 0 || m.height == 0 || m.pixels() > 32_000_000 || !range(m.fps, 0.1, 240.0)) {
        return Err("Invalid camera mode".into());
    }
    Ok(c)
}
pub fn sample(rule: SampleRule) -> Result<SampleRule, String> {
    if !range(rule.min_s, 0.2, 120.0)
        || !range(rule.max_s, rule.min_s, 120.0)
        || rule.target_um.is_some_and(|u| !range(u, 0.0001, 1000.0))
    {
        return Err("Sample duration must be 0.2 to 120 seconds; the target must be positive".into());
    }
    Ok(rule)
}
pub fn range(x: f64, min: f64, max: f64) -> bool {
    x.is_finite() && (min..=max).contains(&x)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn rejects_bad_settings() {
        assert!(finder("{\"strips\":999999}").is_err());
        assert!(sample(SampleRule { min_s: 2.0, max_s: 1.0, target_um: None }).is_err());
        assert!(finder("").is_ok());
    }
}
