//! Renders graph audio to a new WAV file without opening an audio device.

use std::fs::{self, File};
use std::io::{BufWriter, Write};
use std::path::Path;

use patina_engine::clock::{SAMPLE_RATE, frame_due};
use patina_engine::{Checked, Cluster, Control, Mode};

use crate::{interrupt, show, toolchain};

const BLOCK: usize = 256;
const HEADER: u32 = 58;

pub fn samples(seconds: f64) -> Result<u32, String> {
    let count = (seconds * f64::from(SAMPLE_RATE)).round();
    let max = (u32::MAX - (HEADER - 8)) / 4;
    if !count.is_finite() || count < 1.0 || count > f64::from(max) {
        return Err("the duration must fit at least one sample and the WAV size limit".into());
    }
    Ok(count as u32)
}

pub fn run(checked: Checked, out: &Path, seconds: f64, mode: Mode) -> Result<bool, String> {
    let count = samples(seconds)?;
    if out.try_exists().map_err(|e| e.to_string())? {
        return Err(format!("the output already exists: {}", out.display()));
    }
    let toolchain = toolchain()?;
    eprintln!("Building with Rust {}...", toolchain.release);
    let mut cluster = checked.build(&toolchain, mode).map_err(|e| e.to_string())?;
    if !cluster.blocks() || cluster.events() {
        return Err("render needs an audio graph without event nodes".into());
    }
    interrupt::catch();
    let control: &Control = &interrupt::CONTROL;
    control.set_quiet(true);
    if !show(&cluster.set_params(checked.graph())) || !show(&cluster.run(control)) {
        return Ok(false);
    }
    let file = File::create_new(out).map_err(|e| format!("{}: {e}", out.display()))?;
    let mut writer = BufWriter::new(file);
    let result = (|| {
        header(&mut writer, count).map_err(|e| e.to_string())?;
        let complete = write_audio(
            &mut cluster,
            control,
            &mut writer,
            count,
            checked.frame_rate(),
        )?;
        writer.flush().map_err(|e| e.to_string())?;
        Ok(complete)
    })();
    drop(writer);
    if !matches!(result, Ok(true))
        && let Err(e) = fs::remove_file(out)
    {
        return Err(format!(
            "{}; cannot remove the partial file {}: {e}",
            result.err().unwrap_or_else(|| "render stopped".into()),
            out.display()
        ));
    }
    if matches!(result, Ok(true)) {
        eprintln!(
            "Wrote {count} samples at {SAMPLE_RATE} Hz to {}",
            out.display()
        );
    }
    result
}

fn write_audio(
    cluster: &mut Cluster,
    control: &Control,
    writer: &mut impl Write,
    count: u32,
    rate: u32,
) -> Result<bool, String> {
    let (mut left, mut frame, mut played) = (count, 0, false);
    let mut bytes = [0_u8; BLOCK * 4];
    for block in 0..u64::from(count).div_ceil(BLOCK as u64) {
        if control.stopped() {
            return Ok(false);
        }
        while cluster.frames() && frame_due(frame, block, rate) {
            if !show(cluster.frame(control)) {
                return Ok(false);
            }
            frame += 1;
        }
        if !show(cluster.block(control)) {
            return Ok(false);
        }
        let length = (left as usize).min(BLOCK);
        let sound = cluster.sound();
        played |= sound.is_some();
        for (i, target) in bytes[..length * 4]
            .as_chunks_mut::<4>()
            .0
            .iter_mut()
            .enumerate()
        {
            let value = sound.map_or(0.0, |sound| sound[i]);
            if !value.is_finite() {
                return Err(format!(
                    "the graph produced a non-finite sample at {}",
                    u64::from(count - left) + i as u64
                ));
            }
            target.copy_from_slice(&value.to_le_bytes());
        }
        writer
            .write_all(&bytes[..length * 4])
            .map_err(|e| e.to_string())?;
        left -= length as u32;
    }
    if !played {
        return Err("the graph did not send audio with audio::play".into());
    }
    Ok(!control.stopped())
}

fn header(out: &mut impl Write, samples: u32) -> std::io::Result<()> {
    let data = samples * 4;
    out.write_all(b"RIFF")?;
    out.write_all(&(data + (HEADER - 8)).to_le_bytes())?;
    out.write_all(b"WAVEfmt ")?;
    out.write_all(&18_u32.to_le_bytes())?;
    for value in [3_u16, 1] {
        out.write_all(&value.to_le_bytes())?;
    }
    out.write_all(&SAMPLE_RATE.to_le_bytes())?;
    out.write_all(&(SAMPLE_RATE * 4).to_le_bytes())?;
    for value in [4_u16, 32, 0] {
        out.write_all(&value.to_le_bytes())?;
    }
    out.write_all(b"fact")?;
    out.write_all(&4_u32.to_le_bytes())?;
    out.write_all(&samples.to_le_bytes())?;
    out.write_all(b"data")?;
    out.write_all(&data.to_le_bytes())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn duration_rounds_to_samples_and_respects_the_riff_limit() {
        assert_eq!(samples(0.01337).unwrap(), 642);
        for bad in [0.0, -1.0, 0.000001, f64::NAN, f64::INFINITY, 86400.0] {
            assert!(samples(bad).is_err(), "{bad}");
        }
        let max = (u32::MAX - (HEADER - 8)) / 4;
        assert_eq!(samples(f64::from(max) / f64::from(SAMPLE_RATE)), Ok(max));
        assert!(samples(f64::from(max + 1) / f64::from(SAMPLE_RATE)).is_err());
        let mut bytes = Vec::new();
        header(&mut bytes, max).unwrap();
        assert_eq!(bytes.len(), HEADER as usize);
        assert_eq!(
            u32::from_le_bytes(bytes[4..8].try_into().unwrap()),
            max * 4 + (HEADER - 8)
        );
    }
}
