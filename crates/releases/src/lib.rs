//! Releases of a GitHub repository: list them through the public API, pick an
//! asset by name pattern, download it and check its SHA-256.
//!
//! A token is optional and only needed for a private repository. With one,
//! the download goes through the API URL of the asset, which is the only
//! form a private repository serves.

use std::fs;
use std::io::{self, Read, Write};
use std::path::Path;
use std::time::Duration;

use serde::Deserialize;
use sha2::{Digest, Sha256};

const API: &str = "https://api.github.com";
const USER_AGENT: &str = "morphe-releases";

/// One file attached to a release.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct Asset {
    pub name: String,
    pub size: u64,
    /// The API URL; downloads from a private repository use this one.
    pub url: String,
    pub browser_download_url: String,
}

/// One release.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct Release {
    pub tag_name: String,
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default)]
    pub draft: bool,
    #[serde(default)]
    pub prerelease: bool,
    #[serde(default)]
    pub published_at: Option<String>,
    #[serde(default)]
    pub assets: Vec<Asset>,
}

#[derive(Debug)]
pub enum Error {
    /// The request failed or the answer was not what the API gives.
    Http(String),
    /// The answer was not valid JSON for a release list.
    Json(String),
    /// The file on disk could not be written.
    Io(io::Error),
    /// The downloaded bytes do not match the expected hash.
    Checksum { expected: String, actual: String },
}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Error::Http(m) => write!(f, "{m}"),
            Error::Json(m) => write!(f, "the release list could not be read: {m}"),
            Error::Io(e) => write!(f, "{e}"),
            Error::Checksum { expected, actual } => {
                write!(
                    f,
                    "the download does not match its checksum (expected {expected}, got {actual})"
                )
            }
        }
    }
}

impl std::error::Error for Error {}

impl From<io::Error> for Error {
    fn from(e: io::Error) -> Self {
        Error::Io(e)
    }
}

/// Reads the JSON the releases endpoint returns. Drafts are dropped.
pub fn parse_releases(json: &str) -> Result<Vec<Release>, Error> {
    let mut list: Vec<Release> =
        serde_json::from_str(json).map_err(|e| Error::Json(e.to_string()))?;
    list.retain(|r| !r.draft);
    Ok(list)
}

/// True when `name` matches `pattern`, where `*` stands for any run of
/// characters (including none). Case is ignored.
pub fn matches(pattern: &str, name: &str) -> bool {
    let p: Vec<char> = pattern.to_lowercase().chars().collect();
    let n: Vec<char> = name.to_lowercase().chars().collect();
    // Greedy match with one backtrack point.
    let (mut pi, mut ni) = (0, 0);
    let (mut star, mut mark) = (None, 0);
    while ni < n.len() {
        if pi < p.len() && p[pi] == '*' {
            star = Some(pi);
            mark = ni;
            pi += 1;
        } else if pi < p.len() && p[pi] == n[ni] {
            pi += 1;
            ni += 1;
        } else if let Some(s) = star {
            pi = s + 1;
            mark += 1;
            ni = mark;
        } else {
            return false;
        }
    }
    while pi < p.len() && p[pi] == '*' {
        pi += 1;
    }
    pi == p.len()
}

/// The first asset of the release whose name matches the pattern.
pub fn pick_asset<'a>(release: &'a Release, pattern: &str) -> Option<&'a Asset> {
    release.assets.iter().find(|a| matches(pattern, &a.name))
}

/// The newest release that has a matching asset. `prereleases` allows
/// pre-releases; the list is expected newest first, as the API gives it.
pub fn latest_with_asset<'a>(
    releases: &'a [Release],
    pattern: &str,
    prereleases: bool,
) -> Option<(&'a Release, &'a Asset)> {
    releases
        .iter()
        .filter(|r| prereleases || !r.prerelease)
        .find_map(|r| pick_asset(r, pattern).map(|a| (r, a)))
}

/// The hash `name` has in a checksum file (`<hex>  <name>` per line, with an
/// optional `*` before the name, as `sha256sum` writes it).
pub fn checksum_for(sums: &str, name: &str) -> Option<String> {
    sums.lines().find_map(|line| {
        let (hash, file) = line.trim().split_once(char::is_whitespace)?;
        let file = file.trim().trim_start_matches('*');
        (file == name && hash.len() == 64 && hash.bytes().all(|b| b.is_ascii_hexdigit()))
            .then(|| hash.to_lowercase())
    })
}

/// The lowercase SHA-256 of some bytes.
pub fn sha256_hex(bytes: &[u8]) -> String {
    hex(&Sha256::digest(bytes))
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

fn agent() -> ureq::Agent {
    ureq::AgentBuilder::new()
        .timeout_connect(Duration::from_secs(15))
        .timeout_read(Duration::from_secs(60))
        .user_agent(USER_AGENT)
        .build()
}

fn get(url: &str, accept: &str, token: Option<&str>) -> Result<ureq::Response, Error> {
    let mut req = agent().get(url).set("Accept", accept);
    if let Some(t) = token.filter(|t| !t.is_empty()) {
        req = req.set("Authorization", &format!("Bearer {t}"));
    }
    req.call().map_err(|e| match e {
        ureq::Error::Status(404, _) => {
            Error::Http("not found (a private repository needs a token)".into())
        }
        ureq::Error::Status(code, _) => Error::Http(format!("the server answered {code}")),
        other => Error::Http(other.to_string()),
    })
}

/// The releases of `repo` (`owner/name`), newest first.
pub fn list_releases(repo: &str, token: Option<&str>) -> Result<Vec<Release>, Error> {
    let url = format!("{API}/repos/{repo}/releases?per_page=30");
    let body = get(&url, "application/vnd.github+json", token)?
        .into_string()
        .map_err(Error::Io)?;
    parse_releases(&body)
}

/// Downloads an asset to `dest`. With `expected` (a SHA-256 in hex) the file
/// is kept only when it matches; the hash of the bytes is returned. The data
/// goes to a sibling temporary file first, so `dest` never holds a partial
/// download.
pub fn download(
    asset: &Asset,
    token: Option<&str>,
    dest: &Path,
    expected: Option<&str>,
) -> Result<String, Error> {
    let private = token.is_some_and(|t| !t.is_empty());
    let (url, accept) = if private {
        (asset.url.as_str(), "application/octet-stream")
    } else {
        (asset.browser_download_url.as_str(), "*/*")
    };
    let response = get(url, accept, token)?;
    let tmp = dest.with_extension("part");
    let mut hasher = Sha256::new();
    let result = (|| -> Result<(), Error> {
        let mut file = fs::File::create(&tmp)?;
        let mut reader = response.into_reader();
        let mut buf = [0u8; 64 * 1024];
        loop {
            let n = reader.read(&mut buf)?;
            if n == 0 {
                break;
            }
            hasher.update(&buf[..n]);
            file.write_all(&buf[..n])?;
        }
        Ok(())
    })();
    if let Err(e) = result {
        let _ = fs::remove_file(&tmp);
        return Err(e);
    }
    let actual = hex(&hasher.finalize());
    if let Some(want) = expected {
        if !want.eq_ignore_ascii_case(&actual) {
            let _ = fs::remove_file(&tmp);
            return Err(Error::Checksum {
                expected: want.to_lowercase(),
                actual,
            });
        }
    }
    fs::rename(&tmp, dest)?;
    Ok(actual)
}

#[cfg(test)]
mod tests {
    use super::*;

    const JSON: &str = r#"[
      {"tag_name":"v0.3.0","name":"Third","draft":false,"prerelease":true,
       "published_at":"2026-01-03T00:00:00Z",
       "assets":[{"name":"kenon-0.3.0-windows-x64.zip","size":10,"url":"https://api/a/1","browser_download_url":"https://dl/1"}]},
      {"tag_name":"v0.2.1","draft":true,"assets":[]},
      {"tag_name":"v0.2.0","name":"Second","draft":false,"prerelease":false,
       "assets":[
         {"name":"kenon-0.2.0-linux-x64.tar.gz","size":20,"url":"https://api/a/2","browser_download_url":"https://dl/2"},
         {"name":"Kenon-0.2.0-Windows-x64.zip","size":30,"url":"https://api/a/3","browser_download_url":"https://dl/3"},
         {"name":"SHA256SUMS","size":5,"url":"https://api/a/4","browser_download_url":"https://dl/4"}]}
    ]"#;

    #[test]
    fn parses_and_drops_drafts() {
        let list = parse_releases(JSON).unwrap();
        assert_eq!(list.len(), 2);
        assert_eq!(list[0].tag_name, "v0.3.0");
        assert!(list[0].prerelease);
        assert_eq!(list[1].assets.len(), 3);
        assert_eq!(list[1].name.as_deref(), Some("Second"));
        assert!(parse_releases("{}").is_err());
    }

    #[test]
    fn name_patterns() {
        assert!(matches("*windows-x64.zip", "kenon-0.2.0-windows-x64.zip"));
        assert!(matches("*WINDOWS*", "Kenon-0.2.0-Windows-x64.zip"));
        assert!(matches("SHA256SUMS", "sha256sums"));
        assert!(matches("a*b*c", "aXXbYYc"));
        assert!(!matches("a*b*c", "aXXbYY"));
        assert!(!matches("*.zip", "file.tar.gz"));
        assert!(matches("*", ""));
        assert!(!matches("", "x"));
    }

    #[test]
    fn picks_assets() {
        let list = parse_releases(JSON).unwrap();
        let hit = pick_asset(&list[1], "*windows-x64.zip").unwrap();
        assert_eq!(hit.size, 30);
        assert!(pick_asset(&list[1], "*macos*").is_none());
        // The newest release is a pre-release: it counts only when allowed.
        let (r, _) = latest_with_asset(&list, "*windows-x64.zip", true).unwrap();
        assert_eq!(r.tag_name, "v0.3.0");
        let (r, a) = latest_with_asset(&list, "*windows-x64.zip", false).unwrap();
        assert_eq!(r.tag_name, "v0.2.0");
        assert_eq!(a.browser_download_url, "https://dl/3");
        assert!(latest_with_asset(&list, "*.dmg", true).is_none());
    }

    #[test]
    fn checksum_files() {
        let h1 = "a".repeat(64);
        let h2 = "B".repeat(64);
        let sums = format!("{h1}  one.zip\n{h2} *two.zip\nshort  three.zip\n");
        assert_eq!(checksum_for(&sums, "one.zip").as_deref(), Some(h1.as_str()));
        assert_eq!(checksum_for(&sums, "two.zip"), Some("b".repeat(64)));
        assert_eq!(checksum_for(&sums, "three.zip"), None);
        assert_eq!(checksum_for(&sums, "four.zip"), None);
    }

    #[test]
    fn sha256_of_known_input() {
        assert_eq!(
            sha256_hex(b"abc"),
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
        assert_eq!(
            sha256_hex(b""),
            "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
        );
    }
}
