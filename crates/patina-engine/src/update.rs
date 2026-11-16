//! The **Check for updates** button of the About dialog (spec 15.3). Patina reads the newest
//! release of its GitHub project only when the person clicks the button.

use std::path::PathBuf;

use serde::Serialize;
use serde_json::Value;

use crate::Error;
use crate::cargo::{command, run_text};

/// The newest release of the GitHub project. It skips drafts and pre-releases.
const LATEST: &str = "https://api.github.com/repos/bhowiebkr/Patina/releases/latest";
/// The start of the page of each release.
const PAGES: &str = "https://github.com/bhowiebkr/Patina/releases/";

/// The newest release.
#[derive(Debug, PartialEq, Serialize)]
pub struct Release {
    pub version: String,
    /// The page of the release on GitHub.
    pub url: String,
    /// True when the release is newer than the version given to [`newest_release`].
    pub newer: bool,
}

/// Reads the newest release from GitHub and compares it with `current`. `Ok(None)` means that
/// the project has no release yet.
pub fn newest_release(current: &str) -> Result<Option<Release>, Error> {
    let curl = match cfg!(windows) {
        true => crate::install::system32().join("curl.exe"),
        false => PathBuf::from("curl"),
    };
    let text = run_text(
        command(&curl)
            .args(["--silent", "--show-error", "--location", "--max-time", "20"])
            .args(["--proto", "=https", "--proto-redir", "=https"])
            .args(["--header", "Accept: application/vnd.github+json"])
            .arg(LATEST),
    )?;
    let json: Value = serde_json::from_str(&text).map_err(|e| Error::Build(format!("GitHub sent a bad answer: {e}")))?;
    release(&json, current)
}

/// Reads the answer of GitHub. It accepts only a release page of the Patina project.
fn release(json: &Value, current: &str) -> Result<Option<Release>, Error> {
    let (Some(tag), Some(url)) = (json["tag_name"].as_str(), json["html_url"].as_str()) else {
        return match json["message"].as_str() {
            Some("Not Found") => Ok(None),
            Some(message) => Err(Error::Build(format!("GitHub: {message}"))),
            None => Err(Error::Build("GitHub sent no release".into())),
        };
    };
    if !url.starts_with(PAGES) {
        return Err(Error::Build(format!("GitHub sent a page that is not a Patina release: {url}")));
    }
    let version = tag.trim_start_matches(['v', 'V']);
    Ok(Some(Release { version: version.to_owned(), url: url.to_owned(), newer: parts(version) > parts(current) }))
}

/// The numbers of a version without the zeros at the end, for example `[1, 2]` for `1.2.0-beta`.
fn parts(version: &str) -> Vec<u64> {
    let mut parts: Vec<u64> = version.split(['.', '-', '+']).map_while(|p| p.parse().ok()).collect();
    while parts.last() == Some(&0) {
        parts.pop();
    }
    parts
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn reads_the_newest_release_from_the_answer_of_github() {
        let page = "https://github.com/bhowiebkr/Patina/releases/tag/v0.2.0";
        let answer = json!({"tag_name": "v0.2.0", "html_url": page});
        let found = release(&answer, "0.1.0").unwrap().unwrap();
        assert_eq!((found.version.as_str(), found.url.as_str(), found.newer), ("0.2.0", page, true));
        assert!(!release(&answer, "0.2.0").unwrap().unwrap().newer);
        assert!(!release(&answer, "0.10.0").unwrap().unwrap().newer);
        assert!(release(&json!({"tag_name": "v0.10.0", "html_url": page}), "0.9.3").unwrap().unwrap().newer);
        assert!(!release(&json!({"tag_name": "1.0", "html_url": page}), "1.0.0").unwrap().unwrap().newer);
        // No release, an error of GitHub, and a page of another project.
        assert_eq!(release(&json!({"message": "Not Found", "status": "404"}), "0.1.0").unwrap(), None);
        assert!(release(&json!({"message": "API rate limit exceeded"}), "0.1.0").is_err());
        assert!(release(&json!({"tag_name": "v9.0.0", "html_url": "https://example.com/x"}), "0.1.0").is_err());
        assert!(release(&json!([]), "0.1.0").is_err());
    }
}
