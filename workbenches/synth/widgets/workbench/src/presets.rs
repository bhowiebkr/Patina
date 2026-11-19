use crate::{controls::all, parameters::*};
use patina_sdk::Graph;
use std::{
    fs::{self, File},
    io::{Read, Write},
    path::Path,
};

#[derive(Clone, PartialEq)]
pub struct Preset {
    pub name: String,
    pub category: String,
    pub values: Vec<(String, String, String)>,
}

impl Preset {
    pub fn capture(graph: &Graph, name: &str) -> Self {
        let mut values: Vec<_> = all()
            .iter()
            .map(|p| (p.node.into(), p.key.into(), p.read(graph).to_string()))
            .collect();
        for (node, key, default) in [
            (TIMBRE, "shape", "0"),
            (TIMBRE, "octave", "0"),
            (
                SEQUENCE,
                "pattern",
                "-1 -1 -1 -1 -1 -1 -1 -1 -1 -1 -1 -1 -1 -1 -1 -1",
            ),
            (SEQUENCE, "gate", "0.05"),
            (SEQUENCE, "kick", "0"),
            (SEQUENCE, "snare", "0"),
            (SEQUENCE, "hat", "0"),
        ] {
            values.push((
                node.into(),
                key.into(),
                graph.param(node, key).unwrap_or(default).into(),
            ));
        }
        Self {
            name: name.into(),
            category: "User".into(),
            values,
        }
    }
    pub fn set(&mut self, node: &str, key: &str, value: impl ToString) {
        if let Some((_, _, current)) = self
            .values
            .iter_mut()
            .find(|(n, k, _)| n == node && k == key)
        {
            *current = value.to_string();
        }
    }
    pub fn audition(&self) -> [f32; 15] {
        [
            (TIMBRE, "shape"),
            (TIMBRE, "detune"),
            (TIMBRE, "sub"),
            (TIMBRE, "octave"),
            (CONTOUR, "attack"),
            (CONTOUR, "decay"),
            (CONTOUR, "sustain"),
            (CONTOUR, "release"),
            (COLOR, "cutoff"),
            (COLOR, "resonance"),
            (COLOR, "motion"),
            (COLOR, "rate"),
            (SPACE, "seconds"),
            (SPACE, "feedback"),
            (SPACE, "mix"),
        ]
        .map(|(id, key)| {
            self.values
                .iter()
                .find(|(n, k, _)| n == id && k == key)
                .and_then(|(_, _, v)| v.parse().ok())
                .unwrap_or(0.0)
        })
    }
    pub fn apply(&self, graph: &mut Graph) -> Result<(), String> {
        let base = Self::capture(graph, "");
        self.validate(&base)?;
        for (id, key, _) in &self.values {
            let folder = match id.as_str() {
                TIMBRE => "nodes/timbre",
                CONTOUR => "nodes/contour",
                COLOR => "nodes/color",
                SPACE => "nodes/space",
                SEQUENCE => "nodes/sequence",
                CONSOLE => "nodes/console",
                _ => return Err("Unknown preset target".into()),
            };
            if !available(graph, id, folder, key) {
                return Err(format!(
                    "Cannot load: {key} is connected or its node is missing"
                ));
            }
        }
        for (id, key, value) in &self.values {
            if graph.param(id, key) != Some(value.as_str()) {
                graph.set_param(id, key, value);
            }
        }
        Ok(())
    }
    fn validate(&self, base: &Self) -> Result<(), String> {
        if !valid_name(&self.name) || self.values.len() != base.values.len() {
            return Err("Invalid sound name or parameter count".into());
        }
        for (id, key, _) in &base.values {
            let matches: Vec<_> = self
                .values
                .iter()
                .filter(|(n, k, _)| n == id && k == key)
                .collect();
            if matches.len() != 1 {
                return Err("Missing or repeated sound parameter".into());
            }
            let value = &matches[0].2;
            if key == "pattern" {
                let notes: Vec<_> = value.split_whitespace().map(str::parse::<i8>).collect();
                if notes.len() != 16 || notes.iter().any(|n| !matches!(n, Ok(-1..=7))) {
                    return Err("Invalid pattern".into());
                }
            } else {
                let number = value.parse::<f32>().map_err(|_| "Invalid sound value")?;
                let range = all()
                    .iter()
                    .find(|p| p.node == id && p.key == key)
                    .map(|p| p.range)
                    .unwrap_or(match key.as_str() {
                        "shape" => [0.0, 3.0],
                        "octave" => [-2.0, 2.0],
                        "gate" => [0.05, 0.95],
                        _ => [0.0, 65535.0],
                    });
                if !number.is_finite() || number < range[0] || number > range[1] {
                    return Err(format!("Out of range: {key}"));
                }
                if matches!(key.as_str(), "shape" | "octave" | "kick" | "snare" | "hat")
                    && number.fract() != 0.0
                {
                    return Err("An integer parameter has a fraction".into());
                }
            }
        }
        Ok(())
    }
    pub fn save(&self, dir: &Path) -> Result<(), String> {
        self.validate(self)?;
        fs::create_dir_all(dir).map_err(|e| e.to_string())?;
        let path = dir.join(format!("{}.ember", self.name));
        let mut file =
            File::create_new(&path).map_err(|e| format!("Cannot save {}: {e}", path.display()))?;
        let result = (|| {
            writeln!(file, "EMBER1\t{}", self.name)?;
            for (id, key, value) in &self.values {
                writeln!(file, "{id}\t{key}\t{value}")?;
            }
            file.sync_all()
        })();
        drop(file);
        if let Err(error) = result {
            let cleanup = fs::remove_file(&path);
            return Err(format!(
                "Save failed: {error}; partial file cleanup: {cleanup:?}"
            ));
        }
        Ok(())
    }
    pub fn read(path: &Path, base: &Self) -> Result<Self, String> {
        let mut text = String::new();
        File::open(path)
            .map_err(|e| e.to_string())?
            .take(65537)
            .read_to_string(&mut text)
            .map_err(|e| e.to_string())?;
        if text.len() > 65536 {
            return Err("Sound file is too large".into());
        }
        let mut lines = text.lines();
        let name = lines
            .next()
            .and_then(|s| s.strip_prefix("EMBER1\t"))
            .ok_or("Unknown sound format")?;
        let values = lines
            .map(|line| {
                let p: Vec<_> = line.split('\t').collect();
                if p.len() != 3 {
                    return Err("Invalid parameter row".to_string());
                }
                Ok((p[0].into(), p[1].into(), p[2].into()))
            })
            .collect::<Result<Vec<_>, _>>()?;
        let out = Self {
            name: name.into(),
            category: "User".into(),
            values,
        };
        out.validate(base)?;
        Ok(out)
    }
}
pub fn valid_name(name: &str) -> bool {
    !name.is_empty()
        && name.len() <= 48
        && name
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b" -_".contains(&b))
        && !name.ends_with(' ')
}

pub fn factory(graph: &Graph) -> Vec<Preset> {
    [
        ("Night current", "Bass", 1.0, 450.0, 0.1, 0.05),
        ("Glass garden", "Keys", 0.0, 4800.0, 0.02, 0.5),
        ("Copper pulse", "Lead", 2.0, 2200.0, 0.15, 0.2),
        ("Slow lantern", "Pad", 3.0, 1600.0, 0.7, 0.7),
        ("Acid rain", "Bass", 1.0, 900.0, 0.002, 0.12),
        ("Soft orbit", "Keys", 0.0, 7000.0, 0.008, 0.35),
    ]
    .into_iter()
    .map(|(name, category, shape, cutoff, attack, release)| {
        let mut p = Preset::capture(graph, name);
        p.category = category.into();
        for setting in all() {
            p.set(setting.node, setting.key, setting.default);
        }
        p.set(TIMBRE, "shape", shape);
        p.set(TIMBRE, "octave", if category == "Bass" { -1 } else { 0 });
        p.set(COLOR, "cutoff", cutoff);
        p.set(CONTOUR, "attack", attack);
        p.set(CONTOUR, "release", release);
        p.set(SEQUENCE, "pattern", synth_dsp::PATTERNS[0]);
        p.set(SEQUENCE, "gate", 0.58);
        p.set(SEQUENCE, "kick", 4369);
        p.set(SEQUENCE, "snare", 4112);
        p.set(SEQUENCE, "hat", 21845);
        p
    })
    .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn preset_files_round_trip_reject_bad_values_and_preserve_existing_files() {
        let graph = Graph::new("");
        let preset = factory(&graph).remove(0);
        let stamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let dir =
            std::env::temp_dir().join(format!("ember-presets-{}-{stamp}", std::process::id()));
        fs::create_dir(&dir).unwrap();
        let path = dir.join(format!("{}.ember", preset.name));
        preset.save(&dir).unwrap();
        let before = fs::read(&path).unwrap();
        let read = Preset::read(&path, &preset).unwrap();
        assert_eq!(read.values, preset.values);
        assert!(preset.save(&dir).is_err());
        assert_eq!(fs::read(&path).unwrap(), before);
        let mut bad = preset.clone();
        bad.set(COLOR, "cutoff", "NaN");
        assert!(bad.validate(&preset).is_err());
        bad = preset.clone();
        bad.values.push(bad.values[0].clone());
        assert!(bad.validate(&preset).is_err());
        assert!(!valid_name("../outside"));
        assert!(!valid_name("x\ny"));
        fs::remove_file(path).unwrap();
        fs::remove_dir(dir).unwrap();
    }
    #[test]
    fn a_connected_parameter_blocks_the_whole_preset() {
        let mut graph = Graph::new(&format!(
            "node\t{CONSOLE}\tnodes/console\nwire\tx.out\t{CONSOLE}.level"
        ));
        let preset = factory(&graph).remove(0);
        assert!(preset.apply(&mut graph).is_err());
        assert_eq!(graph.edits().count(), 0);
    }
}
