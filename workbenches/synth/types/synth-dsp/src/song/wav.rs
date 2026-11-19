//! Bounded WAV import on the frame thread. Playback never opens a file.
use std::{fs::File, io::Read, path::Path};

pub const MAX_BYTES: usize = 128 * 1024 * 1024;

#[derive(Debug)]
pub struct Sample {
    pub rate: u32,
    pub frames: Vec<[f32; 2]>,
    pub peaks: Vec<f32>,
}

impl Sample {
    pub fn load(path: &Path) -> Result<Self, String> {
        let file = File::open(path).map_err(|e| format!("{}: {e}", path.display()))?;
        if file.metadata().map_err(|e| e.to_string())?.len() > MAX_BYTES as u64 {
            return Err("A WAV file must fit within 128 MiB".into());
        }
        let mut bytes = Vec::new();
        file.take(MAX_BYTES as u64 + 1)
            .read_to_end(&mut bytes)
            .map_err(|e| e.to_string())?;
        Self::decode(&bytes).map_err(|e| format!("{}: {e}", path.display()))
    }

    pub fn decode(bytes: &[u8]) -> Result<Self, String> {
        let bad = || "Unsupported or damaged WAV. Use mono or stereo PCM or float WAV.".to_owned();
        if bytes.len() < 12
            || bytes.len() > MAX_BYTES
            || &bytes[..4] != b"RIFF"
            || &bytes[8..12] != b"WAVE"
        {
            return Err(bad());
        }
        let size = u32::from_le_bytes(bytes[4..8].try_into().unwrap()) as usize;
        let end = size
            .checked_add(8)
            .filter(|n| *n <= bytes.len() && *n >= 12)
            .ok_or_else(bad)?;
        let mut at = 12;
        let (mut format, mut data) = (None, None);
        while at + 8 <= end {
            let size = u32::from_le_bytes(bytes[at + 4..at + 8].try_into().unwrap()) as usize;
            let stop = (at + 8)
                .checked_add(size)
                .filter(|n| *n <= end)
                .ok_or_else(bad)?;
            match &bytes[at..at + 4] {
                b"fmt " if format.is_none() => format = Some(&bytes[at + 8..stop]),
                b"data" if data.is_none() => data = Some(&bytes[at + 8..stop]),
                b"fmt " | b"data" => return Err(bad()),
                _ => {}
            }
            at = stop + (size & 1);
        }
        if at != end {
            return Err(bad());
        }
        let fmt = format.filter(|f| f.len() >= 16).ok_or_else(bad)?;
        let u16_at = |i| u16::from_le_bytes(fmt[i..i + 2].try_into().unwrap());
        let u32_at = |i| u32::from_le_bytes(fmt[i..i + 4].try_into().unwrap());
        let (mut encoding, channels, rate, align, bits) =
            (u16_at(0), u16_at(2), u32_at(4), u16_at(12), u16_at(14));
        if encoding == 0xfffe {
            if fmt.len() < 40
                || u16_at(16) < 22
                || u16_at(18) != bits
                || fmt[26..40] != [0, 0, 0, 0, 16, 0, 128, 0, 0, 170, 0, 56, 155, 113]
            {
                return Err(bad());
            }
            encoding = u16_at(24);
        }
        if !(channels == 1 || channels == 2)
            || !(8000..=192000).contains(&rate)
            || !matches!((encoding, bits), (1, 8 | 16 | 24 | 32) | (3, 32 | 64))
            || align != channels * (bits / 8)
            || u32_at(8) != rate * u32::from(align)
        {
            return Err(bad());
        }
        let data = data
            .filter(|d| !d.is_empty() && d.len().is_multiple_of(usize::from(align)))
            .ok_or_else(bad)?;
        let count = data.len() / usize::from(align);
        if count > MAX_BYTES / 8 {
            return Err("Decoded audio exceeds 128 MiB".into());
        }
        let width = usize::from(bits / 8);
        let mut frames = Vec::with_capacity(count);
        for frame in data.chunks_exact(usize::from(align)) {
            let mut pair = [0.0; 2];
            for (channel, out) in pair.iter_mut().enumerate() {
                let i = channel.min(usize::from(channels) - 1) * width;
                let bytes = &frame[i..i + width];
                *out = match (encoding, bits) {
                    (1, 8) => (f32::from(bytes[0]) - 128.0) / 128.0,
                    (1, 16) => f32::from(i16::from_le_bytes(bytes.try_into().unwrap())) / 32768.0,
                    (1, 24) => {
                        (i32::from_le_bytes([0, bytes[0], bytes[1], bytes[2]]) >> 8) as f32
                            / 8388608.0
                    }
                    (1, 32) => i32::from_le_bytes(bytes.try_into().unwrap()) as f32 / 2147483648.0,
                    (3, 32) => f32::from_le_bytes(bytes.try_into().unwrap()),
                    (3, 64) => f64::from_le_bytes(bytes.try_into().unwrap()) as f32,
                    _ => unreachable!(),
                };
                if !out.is_finite() || out.abs() > 16.0 {
                    return Err("A WAV sample is non-finite or exceeds 16.0".into());
                }
            }
            frames.push(pair);
        }
        let peaks = frames
            .chunks(frames.len().div_ceil(512))
            .flat_map(|chunk| {
                chunk
                    .iter()
                    .flatten()
                    .fold([0.0_f32; 2], |[lo, hi], v| [lo.min(*v), hi.max(*v)])
            })
            .collect();
        Ok(Self {
            rate,
            frames,
            peaks,
        })
    }

    pub fn seconds(&self) -> f64 {
        self.frames.len() as f64 / f64::from(self.rate)
    }

    /// A windowed sinc limits aliasing when a sample plays above its source rate.
    pub fn at(&self, position: f64, step: f64) -> [f32; 2] {
        if position < 0.0 || position >= self.frames.len() as f64 {
            return [0.0; 2];
        }
        let base = position.floor() as isize;
        if step == 1.0 && position.fract() == 0.0 {
            return self.frames[base as usize];
        }
        let cutoff = (1.0 / step).min(1.0);
        let mut sum = [0.0; 2];
        let mut weight = 0.0;
        for tap in -7..=8 {
            let index = base + tap;
            let distance = position - index as f64;
            let x = std::f64::consts::PI * distance * cutoff;
            let sinc = if x.abs() < 1e-9 { 1.0 } else { x.sin() / x };
            let window = 0.5 + 0.5 * (std::f64::consts::PI * distance / 8.0).cos();
            let k = (sinc * window * cutoff) as f32;
            weight += k;
            if let Some(frame) = usize::try_from(index).ok().and_then(|i| self.frames.get(i)) {
                for (out, sample) in sum.iter_mut().zip(frame) {
                    *out += sample * k;
                }
            }
        }
        if weight.abs() > 1e-6 {
            for value in &mut sum {
                *value /= weight;
            }
        }
        sum
    }
}
