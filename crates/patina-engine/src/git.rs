//! The libraries from Git repositories: the tags, the download of a tag and its pin (6.2).

use std::fs;
use std::io::Read;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::Error;
use crate::cargo::{command, run_text};
use crate::update::{curl, parts};

/// The pin of a library to a tag and its commit (6.2).
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct Pin {
    pub url: String,
    pub tag: String,
    pub commit: String,
    /// The SHA-256 hash of the archive, in hex.
    pub sha256: String,
}

/// The tags of a repository against a pin (6.2).
#[derive(Serialize, Debug, PartialEq)]
pub struct Check {
    /// The newest tag, when it is newer than the pinned tag.
    pub newer: Option<String>,
    /// True when the pinned tag points to another commit now.
    pub moved: bool,
}

/// A library of the index (6.2).
#[derive(Serialize, Deserialize, Debug, PartialEq)]
pub struct Entry {
    pub name: String,
    pub url: String,
    #[serde(default)]
    pub description: String,
    pub tag: String,
}

/// The index of the shared libraries, a file of the Patina repository (6.2). A pull request adds a
/// library.
pub const INDEX: &str = "https://raw.githubusercontent.com/bhowiebkr/Patina/main/libraries.toml";

const MAX_FILES: usize = 20_000;
const MAX_BYTES: usize = 512 << 20;
/// The names that Windows keeps for devices. A file with such a name cannot be written.
const DEVICES: &str = "con prn aux nul com1 com2 com3 com4 com5 com6 com7 com8 com9 lpt1 lpt2 lpt3 lpt4 lpt5 lpt6 lpt7 lpt8 lpt9";

/// Writes the answer of a HTTPS GET of a URL to a file.
type Fetch<'a> = &'a dyn Fn(&str, &Path) -> Result<(), Error>;

fn bad(text: impl Into<String>) -> Error {
    Error::Install(text.into())
}

/// Checks the URL of a repository: HTTPS, a host and a path, and no user, query or fragment.
/// Returns it without a `/` or `.git` at the end.
pub fn repository(url: &str) -> Result<String, Error> {
    let url = url.trim().trim_end_matches('/');
    let url = url.strip_suffix(".git").unwrap_or(url);
    let rest = url.strip_prefix("https://").ok_or_else(|| bad("the URL of a library must start with https://"))?;
    let parts: Vec<&str> = rest.split('/').collect();
    let odd = |p: &&str| p.is_empty() || *p == "." || *p == ".." || p.contains(['@', '?', '#', '%', '\\', ' ', ':']);
    if parts.len() < 3 || parts.iter().any(odd) {
        return Err(bad("give the URL of a repository, for example https://github.com/owner/name"));
    }
    Ok(url.to_owned())
}

/// The tags of a repository with their commits, the newest version first (6.2).
pub fn tags(url: &str, work: &Path) -> Result<Vec<(String, String)>, Error> {
    tags_with(&repository(url)?, work, &curl_get)
}

/// Reads the tags of a repository again, and compares them with the pin (6.2).
pub fn check(pin: &Pin, work: &Path) -> Result<Check, Error> {
    Ok(compare(pin, &tags(&pin.url, work)?))
}

/// Downloads the tag `tag` of the repository `url`, or the newest tag when `tag` is empty, into a
/// new folder of `dir`. Returns the folder and the pin (6.2).
pub fn download(url: &str, tag: &str, dir: &Path) -> Result<(PathBuf, Pin), Error> {
    download_with(url, tag, dir, &curl_get)
}

/// Downloads the index and returns its libraries (6.2).
pub fn index(work: &Path) -> Result<Vec<Entry>, Error> {
    index_with(INDEX, work, &curl_get)
}

fn curl_get(url: &str, file: &Path) -> Result<(), Error> {
    run_text(
        command(&curl())
            .args(["--fail", "--silent", "--show-error", "--location", "--retry", "2"])
            .args(["--proto", "=https", "--proto-redir", "=https"])
            .args(["--connect-timeout", "30", "--max-time", "900", "--max-filesize", &MAX_BYTES.to_string(), "--output"])
            .arg(file)
            .arg(url),
    )?;
    Ok(())
}

fn tags_with(url: &str, work: &Path, fetch: Fetch) -> Result<Vec<(String, String)>, Error> {
    fs::create_dir_all(work).map_err(|e| Error::Io(work.to_owned(), e))?;
    let file = work.join("refs");
    let read = fetch(&format!("{url}/info/refs?service=git-upload-pack"), &file).and_then(|()| fs::read(&file).map_err(|e| Error::Io(file.clone(), e)));
    let _ = fs::remove_file(&file);
    parse_refs(&read?)
}

fn index_with(url: &str, work: &Path, fetch: Fetch) -> Result<Vec<Entry>, Error> {
    fs::create_dir_all(work).map_err(|e| Error::Io(work.to_owned(), e))?;
    let file = work.join("libraries.toml");
    let read = fetch(url, &file).and_then(|()| fs::read_to_string(&file).map_err(|e| Error::Io(file.clone(), e)));
    let _ = fs::remove_file(&file);
    parse_index(&read?)
}

/// Reads the index. An entry with a bad URL or tag is left out, so it does not hide the others.
fn parse_index(text: &str) -> Result<Vec<Entry>, Error> {
    #[derive(Deserialize)]
    struct Index {
        #[serde(default)]
        library: Vec<Entry>,
    }
    let index: Index = toml::from_str(text).map_err(|e| bad(format!("the library index is not valid: {e}")))?;
    let good = |mut entry: Entry| {
        entry.url = repository(&entry.url).ok()?;
        tag_name(&entry.tag).then_some(entry)
    };
    Ok(index.library.into_iter().filter_map(good).collect())
}

/// A tag name that Patina accepts: letters, digits and `._+-/`.
fn tag_name(tag: &str) -> bool {
    !tag.is_empty() && tag.bytes().all(|b| b.is_ascii_alphanumeric() || b"._+-/".contains(&b))
}

fn download_with(url: &str, tag: &str, dir: &Path, fetch: Fetch) -> Result<(PathBuf, Pin), Error> {
    let url = repository(url)?;
    let work = dir.join(".download");
    let _ = fs::remove_dir_all(&work);
    let result = fetch_tag(&url, tag.trim(), dir, &work, fetch);
    let _ = fs::remove_dir_all(&work);
    result
}

fn fetch_tag(url: &str, want: &str, dir: &Path, work: &Path, fetch: Fetch) -> Result<(PathBuf, Pin), Error> {
    let tags = tags_with(url, work, fetch)?;
    let found = match want {
        "" => tags.first().ok_or_else(|| bad(format!("{url} has no tags")))?,
        want => tags.iter().find(|(t, _)| t == want).ok_or_else(|| bad(format!("{url} has no tag {want}")))?,
    };
    let (tag, commit) = (found.0.clone(), found.1.clone());
    let name = url.rsplit('/').next().unwrap_or_default();
    let folder = dir.join(format!("{}-{}", safe(name), safe(&tag)));
    if folder.exists() {
        return Err(bad(format!("{} exists. Remove that library first", folder.display())));
    }
    let archive = work.join("archive.tar.gz");
    let path = match url.split('/').nth(2).is_some_and(|host| host.contains("gitlab")) {
        true => format!("{url}/-/archive/{tag}/{name}-{tag}.tar.gz"),
        false => format!("{url}/archive/{tag}.tar.gz"),
    };
    fetch(&path, &archive)?;
    let bytes = fs::read(&archive).map_err(|e| Error::Io(archive.clone(), e))?;
    let sha256 = Sha256::digest(&bytes).iter().map(|b| format!("{b:02x}")).collect();
    let unpacked = work.join("unpack");
    match unpack(&bytes, &unpacked)? {
        Some(c) if c == commit => {}
        Some(c) => return Err(bad(format!("the archive of {tag} has the commit {c}, but the tag points to {commit}"))),
        None => return Err(bad(format!("the archive of {tag} does not give its commit"))),
    }
    if !unpacked.join("patina.toml").is_file() {
        return Err(bad(format!("{url} is not a Patina project: its root has no patina.toml")));
    }
    fs::rename(&unpacked, &folder).map_err(|e| Error::Io(folder.clone(), e))?;
    Ok((folder, Pin { url: url.to_owned(), tag, commit, sha256 }))
}

/// A folder name part: letters, digits, `.`, `_` and `-`, with `-` for each other character.
fn safe(text: &str) -> String {
    text.chars().map(|c| if c.is_ascii_alphanumeric() || "._-".contains(c) { c } else { '-' }).collect()
}

fn version(tag: &str) -> Vec<u64> {
    parts(tag.trim_start_matches(['v', 'V']))
}

fn compare(pin: &Pin, tags: &[(String, String)]) -> Check {
    Check {
        newer: tags.first().filter(|(t, _)| version(t) > version(&pin.tag)).map(|(t, _)| t.clone()),
        moved: tags.iter().any(|(t, c)| *t == pin.tag && *c != pin.commit),
    }
}

/// Reads the tags from the `info/refs` answer of the Git smart HTTP protocol. A peeled entry
/// (`^{}`) gives the commit of an annotated tag.
fn parse_refs(bytes: &[u8]) -> Result<Vec<(String, String)>, Error> {
    let mut tags: Vec<(String, String)> = Vec::new();
    let (mut at, mut service) = (0, false);
    while let Some(head) = bytes.get(at..at + 4) {
        let len = std::str::from_utf8(head).ok().and_then(|h| usize::from_str_radix(h, 16).ok()).ok_or_else(|| bad("the Git host sent a bad list of tags"))?;
        if len < 4 {
            at += 4;
            continue;
        }
        let line = bytes.get(at + 4..at + len).ok_or_else(|| bad("the Git host sent a cut list of tags"))?;
        at += len;
        let line = String::from_utf8_lossy(line);
        let line = line.split('\0').next().unwrap_or_default().trim_end();
        service |= line == "# service=git-upload-pack";
        let Some((commit, name)) = line.split_once(' ') else { continue };
        let Some(tag) = name.strip_prefix("refs/tags/") else { continue };
        if !matches!(commit.len(), 40 | 64) || !commit.bytes().all(|b| b.is_ascii_hexdigit()) {
            continue;
        }
        let (tag, peeled) = tag.strip_suffix("^{}").map_or((tag, false), |t| (t, true));
        if !tag_name(tag) {
            continue;
        }
        match tags.iter_mut().find(|(t, _)| t == tag) {
            Some(entry) if peeled => entry.1 = commit.to_owned(),
            Some(_) => {}
            None => tags.push((tag.to_owned(), commit.to_owned())),
        }
    }
    if !service {
        return Err(bad("the URL does not give a Git repository over HTTPS"));
    }
    tags.sort_by(|a, b| version(&b.0).cmp(&version(&a.0)).then_with(|| b.0.cmp(&a.0)));
    Ok(tags)
}

/// Unpacks a `.tar.gz` archive of `git archive` into `dest`, without its top folder. Returns
/// the commit of the `comment` header.
fn unpack(bytes: &[u8], dest: &Path) -> Result<Option<String>, Error> {
    let mut tar = Vec::new();
    let mut gz = flate2::read::GzDecoder::new(bytes).take(MAX_BYTES as u64 + 1);
    gz.read_to_end(&mut tar).map_err(|e| bad(format!("the archive does not unpack: {e}")))?;
    if tar.len() > MAX_BYTES {
        return Err(bad("the archive has more than 512 MB"));
    }
    let (mut commit, mut long, mut files, mut at) = (None, None, 0, 0);
    while let Some(head) = tar.get(at..at + 512) {
        if head.iter().all(|&b| b == 0) {
            break;
        }
        let size = octal(&head[124..136])?;
        let body = tar.get(at + 512..at + 512 + size).ok_or_else(|| bad("the archive is cut"))?;
        at += 512 + size.div_ceil(512) * 512;
        match head[156] {
            b'g' => commit = pax(body, "comment").or(commit),
            b'x' => long = pax(body, "path"),
            kind @ (0 | b'0' | b'5') => {
                let path = long.take().unwrap_or_else(|| ustar_path(head));
                let Some(target) = place(dest, &path)? else { continue };
                if kind == b'5' {
                    fs::create_dir_all(&target).map_err(|e| Error::Io(target.clone(), e))?;
                    continue;
                }
                files += 1;
                if files > MAX_FILES {
                    return Err(bad("the archive has more than 20000 files"));
                }
                if let Some(parent) = target.parent() {
                    fs::create_dir_all(parent).map_err(|e| Error::Io(parent.to_owned(), e))?;
                }
                fs::write(&target, body).map_err(|e| Error::Io(target.clone(), e))?;
            }
            _ => return Err(bad(format!("the archive has a link or a special file: {}", long.take().unwrap_or_else(|| ustar_path(head))))),
        }
    }
    Ok(commit)
}

/// The place of an archive path in `dest`, without the top folder. None for the top folder.
fn place(dest: &Path, path: &str) -> Result<Option<PathBuf>, Error> {
    let mut parts = path.split('/').filter(|p| !p.is_empty());
    parts.next();
    let mut target = dest.to_owned();
    let mut any = false;
    for part in parts {
        let stem = part.split('.').next().unwrap_or_default().to_ascii_lowercase();
        if part == "." || part == ".." || part.contains([':', '\\']) || DEVICES.split(' ').any(|d| d == stem) || path.starts_with('/') {
            return Err(bad(format!("the archive has an unsafe path: {path}")));
        }
        target.push(part);
        any = true;
    }
    Ok(any.then_some(target))
}

fn octal(field: &[u8]) -> Result<usize, Error> {
    let text = std::str::from_utf8(field).unwrap_or_default().trim_matches(['\0', ' ']);
    usize::from_str_radix(if text.is_empty() { "0" } else { text }, 8).map_err(|_| bad("the archive has a bad size field"))
}

fn ustar_path(head: &[u8]) -> String {
    let text = |field: &[u8]| String::from_utf8_lossy(field.split(|&b| b == 0).next().unwrap_or_default()).into_owned();
    let (name, prefix) = (text(&head[0..100]), text(&head[345..500]));
    if &head[257..262] == b"ustar" && !prefix.is_empty() { format!("{prefix}/{name}") } else { name }
}

/// The value of `key` in the records of a pax header: `<length> <key>=<value>\n`.
fn pax(body: &[u8], key: &str) -> Option<String> {
    let mut rest = body;
    while let Some(space) = rest.iter().position(|&b| b == b' ') {
        let len: usize = std::str::from_utf8(&rest[..space]).ok()?.parse().ok()?;
        let record = std::str::from_utf8(rest.get(space + 1..len)?).ok()?;
        if let Some(value) = record.strip_suffix('\n').and_then(|r| r.strip_prefix(key)).and_then(|r| r.strip_prefix('=')) {
            return Some(value.to_owned());
        }
        rest = &rest[len..];
    }
    None
}

#[cfg(test)]
mod tests;
