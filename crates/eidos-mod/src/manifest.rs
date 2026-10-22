//! A mod folder's manifest and notes: the part of a mod folder that needs no
//! game data.
//!
//! ```text
//! <mod>/eidos_mod.txt        the manifest the port reads (key = value)
//! <mod>/kenon.json           the editor's own notes: which ROM it is built on
//! ```
//!
//! The editor's richer project type (member overrides, changed assets) builds
//! on [`Folder`].

use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

pub const MANIFEST: &str = "eidos_mod.txt";
pub const KENON_FILE: &str = "kenon.json";
/// The range the port accepts for `order` (eidos docs/mods.md).
pub const ORDER_LIMIT: i32 = 1_000_000;

/// The keys of `eidos_mod.txt` the editor edits; every other line is kept.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Manifest {
    pub name: String,
    pub order: i32,
    /// The port refuses a mod whose `api` is not its own interface version
    /// (eidos sdk/mod_manifest.c), so the editor writes it only when the mod
    /// already declares one (a mod with code); a data-only mod has none.
    pub api: Option<u32>,
    /// The lines the editor does not edit, kept in order.
    #[serde(skip)]
    pub other: Vec<String>,
}

impl Manifest {
    pub fn parse(text: &str, default_name: &str) -> Self {
        let mut m = Manifest {
            name: default_name.to_string(),
            order: 0,
            api: None,
            other: Vec::new(),
        };
        for line in text.lines() {
            let body = line.split('#').next().unwrap_or("");
            let Some((key, value)) = body.split_once('=') else {
                m.other.push(line.to_string());
                continue;
            };
            let value = value.trim();
            match key.trim() {
                "name" => m.name = value.to_string(),
                // The port ignores a bad or out-of-range order and keeps 0.
                "order" => {
                    m.order = value
                        .parse()
                        .ok()
                        .filter(|o: &i32| o.abs() <= ORDER_LIMIT)
                        .unwrap_or(0)
                }
                "api" => m.api = value.parse().ok(),
                _ => m.other.push(line.to_string()),
            }
        }
        m
    }

    pub fn write(&self) -> String {
        let mut out = format!(
            "# Written by kenon.\nname = {}\norder = {}\n",
            self.name,
            self.order.clamp(-ORDER_LIMIT, ORDER_LIMIT)
        );
        if let Some(api) = self.api {
            out.push_str(&format!("api = {api}\n"));
        }
        for line in &self.other {
            if !line.starts_with("# Written by kenon.") {
                out.push_str(line);
                out.push('\n');
            }
        }
        out
    }
}

/// The editor's own project notes.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct KenonFile {
    /// The ROM the mod is built on, as the user chose it.
    pub rom: Option<PathBuf>,
}

/// A mod folder seen through its manifest and notes.
pub struct Folder {
    pub root: PathBuf,
    pub manifest: Manifest,
    pub kenon: KenonFile,
}

impl Folder {
    /// Creates a mod in an empty (or new) folder.
    pub fn create(root: &Path, name: &str, rom: Option<&Path>) -> io::Result<Self> {
        if root.exists() && fs::read_dir(root)?.next().is_some() {
            return Err(io::Error::new(
                io::ErrorKind::AlreadyExists,
                "the folder is not empty",
            ));
        }
        fs::create_dir_all(root)?;
        let folder = Folder {
            root: root.to_path_buf(),
            manifest: Manifest::parse("", name),
            kenon: KenonFile {
                rom: rom.map(Path::to_path_buf),
            },
        };
        folder.save_meta()?;
        Ok(folder)
    }

    /// Opens an existing mod folder. A folder without a manifest is still a
    /// mod (the port applies the defaults).
    pub fn open(root: &Path) -> io::Result<Self> {
        if !root.is_dir() {
            return Err(io::Error::new(io::ErrorKind::NotFound, "no such folder"));
        }
        let default_name = root
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_else(|| "mod".to_string());
        let manifest = match fs::read_to_string(root.join(MANIFEST)) {
            Ok(text) => Manifest::parse(&text, &default_name),
            Err(e) if e.kind() == io::ErrorKind::NotFound => Manifest::parse("", &default_name),
            Err(e) => return Err(e),
        };
        let kenon = fs::read(root.join(KENON_FILE))
            .ok()
            .and_then(|b| serde_json::from_slice(&b).ok())
            .unwrap_or_default();
        Ok(Folder {
            root: root.to_path_buf(),
            manifest,
            kenon,
        })
    }

    pub fn save_meta(&self) -> io::Result<()> {
        write_atomic(&self.root.join(MANIFEST), self.manifest.write().as_bytes())?;
        let json = serde_json::to_vec_pretty(&self.kenon).map_err(io::Error::other)?;
        write_atomic(&self.root.join(KENON_FILE), &json)
    }
}

/// Writes through a temporary file and a rename.
pub fn write_atomic(path: &Path, bytes: &[u8]) -> io::Result<()> {
    // A name the port cannot read as a member index ("~25.kenon"), so a
    // crash before the rename leaves nothing it would load.
    let tmp = path.with_file_name(format!(
        "~{}.kenon",
        path.file_name()
            .map_or(String::new(), |n| n.to_string_lossy().into_owned())
    ));
    fs::write(&tmp, bytes)?;
    fs::rename(&tmp, path)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn manifest_keeps_other_lines() {
        let m = Manifest::parse(
            "name = Hard mode\norder = x\n# note\nrule = foo bar\n",
            "dir",
        );
        assert_eq!(m.name, "Hard mode");
        assert_eq!(m.order, 0);
        assert_eq!(m.api, None);
        assert!(!m.write().contains("api"));
        assert_eq!(Manifest::parse("order = 2000000\napi = 3", "d").order, 0);
        assert!(Manifest::parse("api = 3", "d")
            .write()
            .contains("api = 3\n"));
        let text = m.write();
        assert!(text.contains("name = Hard mode\n"));
        assert!(text.contains("rule = foo bar\n"));
        assert!(text.contains("# note\n"));
        assert_eq!(Manifest::parse(&text, "dir").name, "Hard mode");
    }

    #[test]
    fn create_and_reopen() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().join("mymod");
        Folder::create(&root, "My mod", Some(Path::new("black.nds"))).unwrap();
        assert!(root.join(MANIFEST).is_file());
        let again = Folder::open(&root).unwrap();
        assert_eq!(again.manifest.name, "My mod");
        assert_eq!(again.kenon.rom.as_deref(), Some(Path::new("black.nds")));
        assert!(Folder::create(&root, "x", None).is_err());
    }
}
