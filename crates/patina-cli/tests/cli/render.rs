//! The command-line render path, including the frame clock and partial final block.

use std::fs;
use std::path::Path;
use std::process::{Command, Output};

use crate::common::copy;

const FRAME: &str = r#"
#[patina_sdk::node(domain = frame)]
pub struct Counter { value: f32 }
impl Counter {
    pub fn init() -> Self { Self { value: 0.0 } }
    pub fn process(&mut self) -> f32 { self.value += 1.25; self.value }
}
"#;

const AUDIO: &str = r#"
#[patina_sdk::node(domain = audio)]
pub fn source(level: f32, finite: bool, play: bool) {
    let sound = patina_sdk::AudioBlock::from_fn(|i| {
        if finite { level + i as f32 / 4096.0 } else { f32::NAN }
    });
    if play { patina_sdk::audio::play(&sound); }
}
"#;

const GRAPH: &str = r#"format = 1
[[node]]
id = "0000000000000001"
folder = "nodes/render-frame"
[[node]]
id = "0000000000000002"
folder = "nodes/render-audio"
[node.params]
finite = "true"
play = "true"
[[wire]]
from = "0000000000000001.out"
to = "0000000000000002.level"
"#;

fn render(project: &Path, out: &Path, flags: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_patina-cli"))
        .arg("render")
        .arg(project)
        .arg(out)
        .args(["--seconds", "0.03001"])
        .args(flags)
        .output()
        .unwrap()
}

fn success(output: &Output) {
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn render_writes_float_wav_and_preserves_existing_files() {
    let project = copy(
        &Path::new(env!("CARGO_MANIFEST_DIR")).join("../../examples/hello"),
        "render",
    );
    for (name, code) in [("render-frame", FRAME), ("render-audio", AUDIO)] {
        let dir = project.join("nodes").join(name);
        fs::create_dir_all(dir.join("src")).unwrap();
        fs::write(dir.join("Cargo.toml"), format!(
            "[package]\nname = {name:?}\nversion = \"0.1.0\"\nedition = \"2024\"\n[dependencies]\npatina-sdk = {{ workspace = true }}\n"
        )).unwrap();
        fs::write(dir.join("src/lib.rs"), code).unwrap();
    }
    let main = project.join("graphs/main.toml");
    fs::write(&main, GRAPH).unwrap();
    let out = project.join("render audio.wav");
    success(&render(&project, &out, &["--edit"]));
    let bytes = fs::read(&out).unwrap();
    let u32_at = |offset| u32::from_le_bytes(bytes[offset..offset + 4].try_into().unwrap());
    assert_eq!(&bytes[..4], b"RIFF");
    assert_eq!(&bytes[8..16], b"WAVEfmt ");
    assert_eq!(&bytes[20..24], &[3, 0, 1, 0]);
    assert_eq!((u32_at(16), u32_at(24), u32_at(28)), (18, 48000, 192000));
    assert_eq!(&bytes[32..38], &[4, 0, 32, 0, 0, 0]);
    assert_eq!(&bytes[38..42], b"fact");
    assert_eq!((u32_at(42), u32_at(46)), (4, 1440));
    assert_eq!(&bytes[50..54], b"data");
    assert_eq!((u32_at(4), u32_at(54), bytes.len()), (5810, 5760, 5818));
    let audio: Vec<_> = bytes[58..]
        .as_chunks::<4>()
        .0
        .iter()
        .copied()
        .map(f32::from_le_bytes)
        .collect();
    assert!(audio.iter().all(|v| v.is_finite()));
    assert_eq!(audio[0], 1.25);
    assert_eq!(audio[1023], 1.25 + 255.0 / 4096.0);
    assert_eq!(audio[1024], 2.5);
    assert_eq!(audio[1439], 2.5 + 159.0 / 4096.0);
    assert!(!render(&project, &out, &[]).status.success());
    assert_eq!(fs::read(&out).unwrap(), bytes);
    let baked = project.join("baked.wav");
    success(&render(&project, &baked, &[]));
    assert_eq!(fs::read(baked).unwrap(), bytes);

    let failed = project.join("failed.wav");
    for (key, error) in [
        ("finite", "non-finite sample"),
        ("play", "did not send audio"),
    ] {
        fs::write(
            &main,
            GRAPH.replace(&format!("{key} = \"true\""), &format!("{key} = \"false\"")),
        )
        .unwrap();
        let output = render(&project, &failed, &["--edit"]);
        assert!(!output.status.success());
        assert!(String::from_utf8_lossy(&output.stderr).contains(error));
        assert!(!failed.exists());
    }
    fs::write(&main, "format = 1\n").unwrap();
    let output = render(&project, &failed, &["--edit"]);
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("needs an audio graph"));
    assert!(!failed.exists());
    fs::write(
        project.join("graphs/alternate.toml"),
        GRAPH.replacen("format = 1", "format = 1\ntop = true", 1),
    )
    .unwrap();
    success(&render(
        &project,
        &failed,
        &["--graph", "alternate", "--edit"],
    ));
    assert_eq!(fs::read(failed).unwrap(), bytes);
}
