use std::io::Write;

use super::*;

const A: &str = "1111111111111111111111111111111111111111";
const B: &str = "2222222222222222222222222222222222222222";
const C: &str = "3333333333333333333333333333333333333333";

fn pkt(line: &str) -> String {
    format!("{:04x}{line}", line.len() + 4)
}

/// An `info/refs` answer: v1.2.0 is an annotated tag of commit B, v1.10.0 is a light tag.
fn refs() -> Vec<u8> {
    let lines = [
        pkt("# service=git-upload-pack\n"),
        "0000".to_owned(),
        pkt(&format!("{A} HEAD\0multi_ack side-band-64k\n")),
        pkt(&format!("{A} refs/heads/main\n")),
        pkt(&format!("{C} refs/tags/v1.2.0\n")),
        pkt(&format!("{B} refs/tags/v1.2.0^{{}}\n")),
        pkt(&format!("{A} refs/tags/v1.10.0\n")),
        pkt(&format!("{A} refs/tags/bad~name\n")),
        "0000".to_owned(),
    ];
    lines.concat().into_bytes()
}

/// A tar header of `path` with the type `kind` and the size of `body`, then the body.
fn entry(tar: &mut Vec<u8>, path: &str, kind: u8, body: &[u8]) {
    let mut head = [0u8; 512];
    head[..path.len()].copy_from_slice(path.as_bytes());
    head[124..135].copy_from_slice(format!("{:011o}", body.len()).as_bytes());
    head[156] = kind;
    head[257..263].copy_from_slice(b"ustar\0");
    tar.extend_from_slice(&head);
    tar.extend_from_slice(body);
    tar.resize(tar.len().div_ceil(512) * 512, 0);
}

/// A `.tar.gz` like `git archive` writes: a pax `comment` with the commit, then the files.
fn archive(commit: &str, files: &[(&str, u8)]) -> Vec<u8> {
    let mut tar = Vec::new();
    let record = format!(" comment={commit}\n");
    let record = format!("{}{record}", record.len() + 2);
    entry(&mut tar, "pax_global_header", b'g', record.as_bytes());
    for (path, kind) in files {
        entry(&mut tar, path, *kind, if *kind == b'5' { b"" } else { b"format = 1\n" });
    }
    tar.extend_from_slice(&[0; 1024]);
    let mut gz = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::fast());
    gz.write_all(&tar).unwrap();
    gz.finish().unwrap()
}

const GOOD: &[(&str, u8)] = &[("lib-v1.2.0/", b'5'), ("lib-v1.2.0/patina.toml", b'0'), ("lib-v1.2.0/nodes/a/src/lib.rs", b'0')];

/// A fetch that answers the refs and the archive of v1.2.0 of `https://example.org/owner/lib`.
fn host(tar: Vec<u8>) -> impl Fn(&str, &Path) -> Result<(), Error> {
    move |url, file| {
        let body = match url {
            "https://example.org/owner/lib/info/refs?service=git-upload-pack" => refs(),
            "https://example.org/owner/lib/archive/v1.2.0.tar.gz" => tar.clone(),
            _ => return Err(bad(format!("404 {url}"))),
        };
        fs::write(file, body).map_err(|e| Error::Io(file.to_owned(), e))
    }
}

fn dir(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("patina-git-{name}-{}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    dir
}

#[test]
fn the_tags_come_newest_first_with_the_commit_of_an_annotated_tag() {
    let tags = parse_refs(&refs()).unwrap();
    assert_eq!(tags, [("v1.10.0".to_owned(), A.to_owned()), ("v1.2.0".to_owned(), B.to_owned())]);
    assert!(parse_refs(b"<html>not git</html>").is_err());
    assert!(parse_refs(pkt("hello\n").as_bytes()).is_err(), "no service line");

    let pin = Pin { url: String::new(), tag: "v1.2.0".to_owned(), commit: C.to_owned(), sha256: String::new() };
    assert_eq!(compare(&pin, &tags), Check { newer: Some("v1.10.0".to_owned()), moved: true });
    let pin = Pin { tag: "v1.10.0".to_owned(), commit: A.to_owned(), ..pin };
    assert_eq!(compare(&pin, &tags), Check { newer: None, moved: false });
}

#[test]
fn only_a_plain_https_repository_url_passes() {
    assert_eq!(repository(" https://github.com/owner/lib.git/ ").unwrap(), "https://github.com/owner/lib");
    for url in ["http://github.com/owner/lib", "https://github.com/owner", "https://user@github.com/owner/lib", "https://github.com/owner/lib?x=1", "https://github.com/../lib", "https://github.com/owner//lib", "file:///c/lib"] {
        assert!(repository(url).is_err(), "{url}");
    }
}

#[test]
fn a_download_unpacks_the_tag_and_pins_its_commit() {
    let root = dir("good");
    let tar = archive(B, GOOD);
    let (folder, pin) = download_with("https://example.org/owner/lib", "v1.2.0", &root, &host(tar.clone())).unwrap();
    assert_eq!(folder, root.join("lib-v1.2.0"));
    assert_eq!(fs::read_to_string(folder.join("nodes/a/src/lib.rs")).unwrap(), "format = 1\n");
    assert_eq!((pin.tag.as_str(), pin.commit.as_str()), ("v1.2.0", B));
    assert_eq!(pin.sha256, Sha256::digest(&tar).iter().map(|b| format!("{b:02x}")).collect::<String>());
    assert!(!root.join(".download").exists(), "the work folder goes");
    // The same tag again does not replace the copy. The newest tag v1.10.0 has no archive here.
    assert!(download_with("https://example.org/owner/lib", "v1.2.0", &root, &host(tar.clone())).unwrap_err().to_string().contains("exists"));
    assert!(download_with("https://example.org/owner/lib", "", &root, &host(tar.clone())).unwrap_err().to_string().contains("v1.10.0"));
    assert!(download_with("https://example.org/owner/lib", "v9", &root, &host(tar)).unwrap_err().to_string().contains("no tag v9"));
    fs::remove_dir_all(&root).unwrap();
}

#[test]
fn a_bad_archive_is_refused_and_leaves_nothing() {
    let root = dir("bad");
    let refuse = |tar: Vec<u8>, want: &str| {
        let error = download_with("https://example.org/owner/lib", "v1.2.0", &root, &host(tar)).unwrap_err().to_string();
        assert!(error.contains(want), "{error}");
        assert_eq!(fs::read_dir(&root).unwrap().count(), 0, "{want}");
    };
    refuse(archive(C, GOOD), "points to");
    refuse(archive(B, &[("lib/", b'5'), ("lib/../evil.rs", b'0')]), "unsafe path");
    refuse(archive(B, &[("lib/", b'5'), ("lib/nodes/con.rs", b'0')]), "unsafe path");
    refuse(archive(B, &[("lib/", b'5'), ("lib/patina.toml", b'0'), ("lib/link", b'2')]), "link");
    refuse(archive(B, &[("lib/", b'5'), ("lib/README.md", b'0')]), "no patina.toml");
    refuse(b"not gzip".to_vec(), "does not unpack");
    fs::remove_dir_all(&root).unwrap();
}
