//! katalogos: an eidos mod's release files, and the check of the katalogos
//! mod index (Project-Holon/katalogos: index.json, the official mods, the
//! workflows that run this tool; the format is its docs/mod-json.md).
//!
//!   katalogos pack <mod-dir> --repo <owner/name> --version <x.y.z> [--out <dir>]
//!   katalogos pack <mod-dir> --official <id> --version <x.y.z> [--out <dir>]
//!       packs the mod folder into <out>/mod.bw and writes <out>/mod.json
//!       (docs/mod-json.md), the two files a mod's release carries.  An
//!       official mod is one of this repository's own (mods/<id>).
//!   katalogos check <index.json>
//!       reads every listed mod's mod.json and mod.bw where the launcher
//!       reads them, and checks them: the format, the hash, the size, the
//!       package's own limits.  An official mod's version must be its
//!       folder's; a version not released yet is packed from the folder
//!       and checked the same way.  Exits 1 if any mod fails.
//!   katalogos pending <index.json>
//!       prints `<id> <version>` for each official mod whose release does
//!       not exist yet (the publish workflow makes them).
//!   katalogos inspect <mod.json> <mod.bw>
//!       the same checks on files already on disk.

use std::fs;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

/// The mod.json format this tool writes and reads.
const FORMAT: u32 = 1;
/// The package's file name in a release.
const PACKAGE: &str = "mod.bw";
/// The cartridge codes a manifest's `game` may name (docs/mod-json.md).
const GAMES: &[(&str, &str)] = &[("IRB", "black"), ("IRA", "white")];
/// The index repository: the official mods live in it (mods/<id>).
pub const INDEX_REPO: &str = "Project-Holon/katalogos";
/// The id prefix of an official mod.
const OFFICIAL_PREFIX: &str = "katalogos/";

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct Package {
    pub file: String,
    pub sha256: String,
    pub size: u64,
    pub files: usize,
    pub unpacked: u64,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct ModJson {
    pub format: u32,
    pub id: String,
    pub name: String,
    pub version: String,
    pub author: String,
    pub description: String,
    pub repo: String,
    /// An official mod's folder in this repository (`mods/<id>`).
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub path: String,
    #[serde(default)]
    pub official: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub multiplayer: Option<u32>,
    pub games: Vec<String>,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub game_code: String,
    #[serde(default)]
    pub tags: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub api: Option<u32>,
    pub has_code: bool,
    pub rules: usize,
    pub package: Package,
}

/// One line of the index: an official mod of this repository (`mod`, with
/// its `version`), or another repository (`repo`, newest release unless a
/// `version` pins one).
#[derive(Deserialize, Debug, Default)]
pub struct Entry {
    #[serde(default, rename = "mod")]
    pub official: Option<String>,
    #[serde(default)]
    pub repo: Option<String>,
    #[serde(default)]
    pub version: Option<String>,
}

impl Entry {
    /// The name the check prints.
    pub fn label(&self) -> String {
        match (&self.official, &self.repo) {
            (Some(id), _) => format!("{OFFICIAL_PREFIX}{id}"),
            (None, Some(r)) => r.clone(),
            _ => "(empty entry)".into(),
        }
    }
}

/// An official mod's id: lower-case letters, digits and `-`.
pub fn valid_id(id: &str) -> bool {
    !id.is_empty()
        && id.len() <= 48
        && id
            .chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-')
        && !id.starts_with('-')
        && !id.ends_with('-')
}

/// An official mod's release tag.
pub fn official_tag(id: &str, version: &str) -> String {
    format!("{id}-v{}", version.trim_start_matches('v'))
}

#[derive(Deserialize, Debug)]
pub struct Index {
    pub format: u32,
    pub mods: Vec<Entry>,
}

fn sha256_hex(bytes: &[u8]) -> String {
    let d = Sha256::digest(bytes);
    d.iter().map(|b| format!("{b:02x}")).collect()
}

/// `owner/name`, each part made of letters, digits, `-`, `_` and `.`.
pub fn valid_repo(repo: &str) -> bool {
    let parts: Vec<&str> = repo.split('/').collect();
    parts.len() == 2
        && parts.iter().all(|p| {
            !p.is_empty()
                && p.chars()
                    .all(|c| c.is_ascii_alphanumeric() || "-_.".contains(c))
                && *p != "."
                && *p != ".."
        })
}

/// x.y.z, digits only (a leading `v` is dropped).
pub fn version_of(tag: &str) -> Option<String> {
    let v = tag.trim().trim_start_matches(['v', 'V']);
    let parts: Vec<&str> = v.split('.').collect();
    if parts.len() == 3
        && parts
            .iter()
            .all(|p| !p.is_empty() && p.len() <= 6 && p.chars().all(|c| c.is_ascii_digit()))
    {
        Some(v.to_string())
    } else {
        None
    }
}

/// How many `rule = ...` lines, whether a `code = ...` line, and the
/// `multiplayer = <n>` number the manifest has.
fn manifest_counts(text: &str) -> (usize, bool, Option<u32>) {
    let mut rules = 0;
    let mut code = false;
    let mut multiplayer = None;
    for line in text.lines() {
        let l = line.split('#').next().unwrap_or("");
        if let Some((k, v)) = l.split_once('=') {
            match k.trim() {
                "rule" if !v.trim().is_empty() => rules += 1,
                "code" if !v.trim().is_empty() => code = true,
                // as the port reads it; a bad number is no number
                "multiplayer" => multiplayer = eidos_mod::modcode::parse_multiplayer(v.trim()).ok(),
                _ => {}
            }
        }
    }
    (rules, code, multiplayer)
}

/// Where a mod comes from: another repository, or this one's mods/<id>.
#[derive(Debug, Clone)]
pub enum Origin {
    Repo(String),
    Official(String),
}

fn games_of(code: &str) -> Result<Vec<String>, String> {
    if code.is_empty() {
        return Ok(GAMES.iter().map(|(_, g)| g.to_string()).collect());
    }
    for (prefix, g) in GAMES {
        if code.len() == 4 && code.starts_with(prefix) {
            return Ok(vec![g.to_string()]);
        }
    }
    Err(format!(
        "the manifest's game = {code} is not a Black or White cartridge code (IRBx, IRAx)"
    ))
}

/// Packs `dir` into `out/mod.bw` and writes `out/mod.json`.
pub fn pack(dir: &Path, origin: &Origin, version: &str, out: &Path) -> Result<ModJson, String> {
    let (repo, id, path, official) = match origin {
        Origin::Repo(r) => {
            if !valid_repo(r) {
                return Err(format!("{r} is not owner/name"));
            }
            (r.clone(), r.to_lowercase(), String::new(), false)
        }
        Origin::Official(id) => {
            if !valid_id(id) {
                return Err(format!("{id} is not an official mod id (a-z, 0-9, -)"));
            }
            (
                INDEX_REPO.to_string(),
                format!("{OFFICIAL_PREFIX}{id}"),
                format!("mods/{id}"),
                true,
            )
        }
    };
    let repo = repo.as_str();
    let version =
        version_of(version).ok_or_else(|| format!("{version} is not a version (x.y.z)"))?;
    let manifest_path = dir.join(eidos_mod::manifest::MANIFEST);
    let text = fs::read_to_string(&manifest_path).map_err(|e| {
        format!(
            "{}: {e} (a mod needs its eidos_mod.txt)",
            manifest_path.display()
        )
    })?;
    let repo_name = match origin {
        Origin::Official(id) => id.as_str(),
        Origin::Repo(_) => repo.split('/').nth(1).unwrap_or(repo),
    };
    let m = eidos_mod::manifest::Manifest::parse(&text, repo_name);
    if !m.version.is_empty() && version_of(&m.version).as_deref() != Some(version.as_str()) {
        return Err(format!(
            "the manifest says version = {}, the release is {version}: make them the same",
            m.version
        ));
    }
    if m.description.trim().is_empty() {
        return Err("the manifest has no description = ... line (the index shows it)".into());
    }
    let games = games_of(m.game.trim())?;
    let (rules, has_code, multiplayer) = manifest_counts(&text);
    fs::create_dir_all(out).map_err(|e| e.to_string())?;
    let bw = out.join(PACKAGE);
    eidos_mod::package::pack(dir, &bw).map_err(|e| format!("packing: {e}"))?;
    let info = eidos_mod::package::inspect(&bw)
        .map_err(|e| format!("the package does not read back: {e}"))?;
    let bytes = fs::read(&bw).map_err(|e| e.to_string())?;
    let owner = repo.split('/').next().unwrap_or("");
    let j = ModJson {
        format: FORMAT,
        id,
        name: m.name.clone(),
        version,
        author: if m.author.trim().is_empty() {
            owner.to_string()
        } else {
            m.author.trim().to_string()
        },
        description: m.description.trim().to_string(),
        repo: repo.to_string(),
        path,
        official,
        multiplayer,
        games,
        game_code: m.game.trim().to_string(),
        tags: m.tags.clone(),
        api: m.api,
        has_code,
        rules,
        package: Package {
            file: PACKAGE.into(),
            sha256: sha256_hex(&bytes),
            size: bytes.len() as u64,
            files: info.files.len(),
            unpacked: info.files.iter().map(|f| f.size).sum(),
        },
    };
    let text = serde_json::to_string_pretty(&j).map_err(|e| e.to_string())? + "\n";
    fs::write(out.join("mod.json"), text).map_err(|e| e.to_string())?;
    Ok(j)
}

/// The checks on a mod.json and its package's bytes.
pub fn verify(j: &ModJson, bw_bytes: &[u8], tmp: &Path) -> Result<(), String> {
    if j.format != FORMAT {
        return Err(format!(
            "mod.json format {} (this check knows {FORMAT})",
            j.format
        ));
    }
    let id_ok = if j.official {
        j.repo == INDEX_REPO
            && j.id
                .strip_prefix(OFFICIAL_PREFIX)
                .is_some_and(|id| valid_id(id) && j.path == format!("mods/{id}"))
    } else {
        valid_repo(&j.repo) && j.id == j.repo.to_lowercase() && j.path.is_empty()
    };
    if !id_ok {
        return Err(format!(
            "repo {} / id {} / path {} do not agree",
            j.repo, j.id, j.path
        ));
    }
    if version_of(&j.version).as_deref() != Some(j.version.as_str()) {
        return Err(format!("version {} is not x.y.z", j.version));
    }
    if j.name.trim().is_empty() || j.description.trim().is_empty() {
        return Err("no name or no description".into());
    }
    if j.games.is_empty() || j.games.iter().any(|g| g != "black" && g != "white") {
        return Err(format!("games {:?}: black and/or white", j.games));
    }
    if j.package.file != PACKAGE {
        return Err(format!(
            "the package is {} (it must be {PACKAGE})",
            j.package.file
        ));
    }
    if bw_bytes.len() as u64 != j.package.size {
        return Err(format!(
            "mod.bw is {} bytes, mod.json says {}",
            bw_bytes.len(),
            j.package.size
        ));
    }
    let h = sha256_hex(bw_bytes);
    if h != j.package.sha256 {
        return Err(format!(
            "mod.bw's SHA-256 is {h}, mod.json says {}",
            j.package.sha256
        ));
    }
    let p = tmp.join("check.bw");
    fs::write(&p, bw_bytes).map_err(|e| e.to_string())?;
    let info = eidos_mod::package::inspect(&p)
        .map_err(|e| format!("mod.bw does not read as a package: {e}"))?;
    if info.files.len() != j.package.files {
        return Err(format!(
            "mod.bw holds {} files, mod.json says {}",
            info.files.len(),
            j.package.files
        ));
    }
    Ok(())
}

fn fetch(url: &str, max: u64) -> Result<Vec<u8>, String> {
    let resp = ureq::get(url)
        .set("User-Agent", "katalogos-check")
        .call()
        .map_err(|e| format!("{url}: {e}"))?;
    let mut buf = Vec::new();
    resp.into_reader()
        .take(max + 1)
        .read_to_end(&mut buf)
        .map_err(|e| format!("{url}: {e}"))?;
    if buf.len() as u64 > max {
        return Err(format!("{url}: larger than {max} bytes"));
    }
    Ok(buf)
}

/// Where the launcher reads a listed mod's files from.
pub fn release_url(e: &Entry, file: &str) -> String {
    match (&e.official, &e.repo, &e.version) {
        (Some(id), _, Some(v)) => format!(
            "https://github.com/{INDEX_REPO}/releases/download/{}/{file}",
            official_tag(id, v)
        ),
        (None, Some(r), Some(v)) => {
            format!(
                "https://github.com/{r}/releases/download/v{}/{file}",
                v.trim_start_matches('v')
            )
        }
        (None, Some(r), None) => format!("https://github.com/{r}/releases/latest/download/{file}"),
        _ => String::new(),
    }
}

/// The index file, read and its entries' shapes checked.
fn read_index(index_path: &Path) -> Result<Index, String> {
    let text =
        fs::read_to_string(index_path).map_err(|e| format!("{}: {e}", index_path.display()))?;
    let idx: Index = serde_json::from_str(&text).map_err(|e| format!("index.json: {e}"))?;
    if idx.format != FORMAT {
        return Err(format!(
            "index format {} (this check knows {FORMAT})",
            idx.format
        ));
    }
    Ok(idx)
}

/// An entry's own shape: one of `mod` and `repo`, a valid name, an x.y.z
/// version (required for an official mod).
fn entry_shape(e: &Entry) -> Result<(), String> {
    match (&e.official, &e.repo) {
        (Some(id), None) => {
            if !valid_id(id) {
                return Err(format!("{id} is not an official mod id (a-z, 0-9, -)"));
            }
            let v = e
                .version
                .as_deref()
                .ok_or("an official mod needs its version")?;
            version_of(v).ok_or("the version is not x.y.z")?;
        }
        (None, Some(r)) => {
            if !valid_repo(r) {
                return Err("not owner/name".into());
            }
            if let Some(v) = &e.version {
                version_of(v).ok_or("the pinned version is not x.y.z")?;
            }
        }
        _ => return Err("an entry has either `mod` (official) or `repo`".into()),
    }
    Ok(())
}

/// Whether a release's file can be read (a HEAD-like probe: a GET that
/// stops after the answer).
fn exists(url: &str) -> bool {
    ureq::get(url)
        .set("User-Agent", "katalogos-check")
        .call()
        .is_ok()
}

/// An official mod's folder (`<root>/mods/<id>`) and the folder its
/// package is made from (`mod/` in it).
fn official_dirs(root: &Path, id: &str) -> (PathBuf, PathBuf) {
    let d = root.join("mods").join(id);
    let m = d.join("mod");
    (d, m)
}

/// The version an official mod's manifest states.
fn official_manifest_version(root: &Path, id: &str) -> Result<String, String> {
    let (_, m) = official_dirs(root, id);
    let p = m.join(eidos_mod::manifest::MANIFEST);
    let text = fs::read_to_string(&p).map_err(|e| format!("{}: {e}", p.display()))?;
    let man = eidos_mod::manifest::Manifest::parse(&text, id);
    version_of(&man.version)
        .ok_or_else(|| format!("{}: version = {} is not x.y.z", p.display(), man.version))
}

fn check(index_path: &Path) -> Result<usize, String> {
    let idx = read_index(index_path)?;
    let root = index_path
        .parent()
        .map(Path::to_path_buf)
        .unwrap_or_default();
    let tmp = std::env::temp_dir().join(format!("katalogos-check-{}", std::process::id()));
    fs::create_dir_all(&tmp).map_err(|e| e.to_string())?;
    let mut seen = std::collections::HashSet::new();
    let mut failed = 0;
    for e in &idx.mods {
        let r = (|| -> Result<(ModJson, &'static str), String> {
            entry_shape(e)?;
            if !seen.insert(e.label().to_lowercase()) {
                return Err("listed twice".into());
            }
            if let Some(id) = &e.official {
                let v = e.version.clone().unwrap_or_default();
                let mv = official_manifest_version(&root, id)?;
                if mv != v.trim_start_matches('v') {
                    return Err(format!("the index says {v}, mods/{id}/mod says {mv}"));
                }
                if !exists(&release_url(e, "mod.json")) {
                    // Not released yet: the publish workflow makes it once
                    // this is on main.  Check what it will publish.
                    let (_, m) = official_dirs(&root, id);
                    let out = tmp.join(id);
                    let j = pack(&m, &Origin::Official(id.clone()), &v, &out)?;
                    let bw = fs::read(out.join(PACKAGE)).map_err(|e| e.to_string())?;
                    verify(&j, &bw, &tmp)?;
                    return Ok((j, " [official, not released yet]"));
                }
            }
            let j: ModJson = serde_json::from_slice(&fetch(&release_url(e, "mod.json"), 1 << 20)?)
                .map_err(|err| format!("mod.json: {err}"))?;
            if j.id != e.label().to_lowercase() {
                return Err(format!("its mod.json is another mod's ({})", j.id));
            }
            let bw = fetch(&release_url(e, PACKAGE), eidos_mod::package::MAX_TOTAL)?;
            verify(&j, &bw, &tmp)?;
            Ok((
                j,
                if e.official.is_some() {
                    " [official]"
                } else {
                    ""
                },
            ))
        })();
        match r {
            Ok((j, note)) => println!("ok   {} {} ({}){note}", e.label(), j.version, j.name),
            Err(err) => {
                failed += 1;
                println!("FAIL {}: {err}", e.label());
            }
        }
    }
    let _ = fs::remove_dir_all(&tmp);
    if failed > 0 {
        Err(format!("{failed} of {} mods failed", idx.mods.len()))
    } else {
        Ok(idx.mods.len())
    }
}

/// The official mods whose listed version has no release yet.
fn pending(index_path: &Path) -> Result<Vec<(String, String)>, String> {
    let idx = read_index(index_path)?;
    let mut out = Vec::new();
    for e in &idx.mods {
        if let (Some(id), Some(v)) = (&e.official, &e.version) {
            entry_shape(e)?;
            if !exists(&release_url(e, "mod.json")) {
                out.push((id.clone(), v.trim_start_matches('v').to_string()));
            }
        }
    }
    Ok(out)
}

fn arg(args: &[String], name: &str) -> Option<String> {
    args.iter()
        .position(|a| a == name)
        .and_then(|i| args.get(i + 1))
        .cloned()
}

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().collect();
    let r = match args.get(1).map(String::as_str) {
        Some("pack") if args.len() >= 3 => {
            let dir = PathBuf::from(&args[2]);
            let origin = match (arg(&args, "--official"), arg(&args, "--repo")) {
                (Some(id), None) => Some(Origin::Official(id)),
                (None, Some(r)) => Some(Origin::Repo(r)),
                _ => None,
            };
            let version = arg(&args, "--version").unwrap_or_default();
            let out = PathBuf::from(arg(&args, "--out").unwrap_or_else(|| "dist".into()));
            match origin {
                None => Err("pack takes one of --repo owner/name and --official <id>".into()),
                Some(o) => pack(&dir, &o, &version, &out).map(|j| {
                    println!("{} {}: {} files, {} bytes, sha256 {}", j.name, j.version, j.package.files, j.package.size, j.package.sha256);
                }),
            }
        }
        Some("check") if args.len() >= 3 => check(Path::new(&args[2])).map(|n| println!("all {n} mods ok")),
        Some("pending") if args.len() >= 3 => pending(Path::new(&args[2])).map(|list| {
            for (id, v) in list {
                println!("{id} {v}");
            }
        }),
        Some("inspect") if args.len() >= 4 => (|| {
            let j: ModJson = serde_json::from_slice(&fs::read(&args[2]).map_err(|e| e.to_string())?)
                .map_err(|e| format!("mod.json: {e}"))?;
            let bw = fs::read(&args[3]).map_err(|e| e.to_string())?;
            let tmp = std::env::temp_dir();
            verify(&j, &bw, &tmp).map(|_| println!("ok {} {}", j.name, j.version))
        })(),
        _ => Err("usage: katalogos pack <mod-dir> (--repo owner/name | --official <id>) --version x.y.z [--out dir]\n       katalogos check <index.json>\n       katalogos pending <index.json>\n       katalogos inspect <mod.json> <mod.bw>".into()),
    };
    match r {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("katalogos: {e}");
            ExitCode::FAILURE
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample(dir: &Path, extra: &str) {
        fs::create_dir_all(dir.join("rules")).unwrap();
        fs::write(
            dir.join("eidos_mod.txt"),
            format!("name = Sample\ndescription = A sample rule.\nrule = battle.faint rules/sample.rule\n{extra}"),
        )
        .unwrap();
        fs::write(dir.join("rules/sample.rule"), "language 1\nrule sample on battle.faint\n    when event.side == 0\n    do message \"Fainted.\"\n").unwrap();
    }

    fn repo(r: &str) -> Origin {
        Origin::Repo(r.into())
    }

    #[test]
    fn packs_and_verifies() {
        let t = tempfile::tempdir().unwrap();
        let m = t.path().join("mod");
        sample(&m, "tags = Challenge, rules\nversion = 1.2.0\n");
        let out = t.path().join("dist");
        let j = pack(&m, &repo("someone/sample"), "v1.2.0", &out).unwrap();
        assert_eq!(j.id, "someone/sample");
        assert_eq!(j.version, "1.2.0");
        assert_eq!(j.author, "someone");
        assert_eq!(j.games, vec!["black", "white"]);
        assert_eq!(j.tags, vec!["challenge", "rules"]);
        assert_eq!(j.rules, 1);
        assert!(!j.has_code);
        assert!(!j.official && j.path.is_empty() && j.multiplayer.is_none());
        let back: ModJson =
            serde_json::from_str(&fs::read_to_string(out.join("mod.json")).unwrap()).unwrap();
        assert_eq!(back, j);
        let bw = fs::read(out.join("mod.bw")).unwrap();
        verify(&back, &bw, t.path()).unwrap();
        // a changed byte is caught
        let mut bad = bw.clone();
        let n = bad.len() / 2;
        bad[n] ^= 0xFF;
        assert!(verify(&back, &bad, t.path()).is_err());
        // packing twice gives the same bytes
        let out2 = t.path().join("dist2");
        let j2 = pack(&m, &repo("someone/sample"), "1.2.0", &out2).unwrap();
        assert_eq!(j2.package.sha256, j.package.sha256);
    }

    #[test]
    fn official_mods() {
        let t = tempfile::tempdir().unwrap();
        let m = t.path().join("mod");
        sample(&m, "version = 1.0.0\nmultiplayer = 1\n");
        let out = t.path().join("dist");
        let j = pack(&m, &Origin::Official("soul-link".into()), "1.0.0", &out).unwrap();
        assert_eq!(j.id, "katalogos/soul-link");
        assert_eq!(j.repo, INDEX_REPO);
        assert_eq!(j.path, "mods/soul-link");
        assert!(j.official);
        assert_eq!(j.multiplayer, Some(1));
        let bw = fs::read(out.join("mod.bw")).unwrap();
        verify(&j, &bw, t.path()).unwrap();
        // an external mod cannot claim to be official
        let mut fake = j.clone();
        fake.repo = "someone/x".into();
        assert!(verify(&fake, &bw, t.path()).is_err());
        let mut fake2 = j.clone();
        fake2.path = "mods/other".into();
        assert!(verify(&fake2, &bw, t.path()).is_err());
        assert!(pack(&m, &Origin::Official("Bad_Id".into()), "1.0.0", &out).is_err());
    }

    #[test]
    fn refuses_mismatches() {
        let t = tempfile::tempdir().unwrap();
        let m = t.path().join("mod");
        sample(&m, "version = 1.0.0\n");
        assert!(pack(&m, &repo("someone/x"), "1.0.1", &t.path().join("o")).is_err());
        assert!(pack(&m, &repo("not-a-repo"), "1.0.0", &t.path().join("o")).is_err());
        assert!(pack(&m, &repo("someone/x"), "1.0", &t.path().join("o")).is_err());
        let w = t.path().join("white");
        sample(&w, "game = IRAO\n");
        assert_eq!(
            pack(&w, &repo("a/b"), "1.0.0", &t.path().join("o2"))
                .unwrap()
                .games,
            vec!["white"]
        );
        let bad = t.path().join("bad");
        sample(&bad, "game = CPUE\n");
        assert!(pack(&bad, &repo("a/b"), "1.0.0", &t.path().join("o3")).is_err());
    }

    #[test]
    fn urls_and_names() {
        assert!(valid_repo("someone/eidos-mod.x_1"));
        assert!(!valid_repo("someone"));
        assert!(!valid_repo("a/b/c"));
        assert!(!valid_repo("a/.."));
        assert!(valid_id("soul-link") && !valid_id("-x") && !valid_id("A") && !valid_id(""));
        let e = Entry {
            repo: Some("a/b".into()),
            ..Default::default()
        };
        assert_eq!(
            release_url(&e, "mod.json"),
            "https://github.com/a/b/releases/latest/download/mod.json"
        );
        let p = Entry {
            repo: Some("a/b".into()),
            version: Some("v1.0.0".into()),
            ..Default::default()
        };
        assert_eq!(
            release_url(&p, "mod.bw"),
            "https://github.com/a/b/releases/download/v1.0.0/mod.bw"
        );
        let o = Entry {
            official: Some("nuzlocke".into()),
            version: Some("1.0.0".into()),
            ..Default::default()
        };
        assert_eq!(
            release_url(&o, "mod.bw"),
            "https://github.com/Project-Holon/katalogos/releases/download/nuzlocke-v1.0.0/mod.bw"
        );
        assert!(entry_shape(&o).is_ok());
        let both = Entry {
            official: Some("x".into()),
            repo: Some("a/b".into()),
            version: Some("1.0.0".into()),
        };
        assert!(entry_shape(&both).is_err());
        let unversioned = Entry {
            official: Some("x".into()),
            ..Default::default()
        };
        assert!(entry_shape(&unversioned).is_err());
        let idx: Index = serde_json::from_str(
            r#"{"format":1,"mods":[{"mod":"nuzlocke","version":"1.0.0"},{"repo":"a/b"}]}"#,
        )
        .unwrap();
        assert_eq!(idx.mods[0].label(), "katalogos/nuzlocke");
        assert_eq!(idx.mods[1].label(), "a/b");
    }
}
