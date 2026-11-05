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
    /// What the mod does, in a sentence or two for players. Empty: none.
    /// The port ignores this key and the three below (it skips keys it does
    /// not know); the editor, the launcher and the mod index read them.
    pub description: String,
    pub author: String,
    pub version: String,
    /// The game code of the cartridge the mod is made for (`IRAF`: White,
    /// French). Empty: not said.
    pub game: String,
    /// Words that sort the mod in the mod index (`tags = hard, rules`):
    /// lowercased, without commas or empty ones. The port ignores the key.
    pub tags: Vec<String>,
    /// The mod's picture in the launcher and the mod index. A value that
    /// does not read stays in `other`, untouched.
    pub icon: Option<Icon>,
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
            description: String::new(),
            author: String::new(),
            version: String::new(),
            game: String::new(),
            tags: Vec::new(),
            icon: None,
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
                "description" => m.description = value.to_string(),
                "author" => m.author = value.to_string(),
                "version" => m.version = value.to_string(),
                "game" => m.game = value.to_string(),
                "tags" => m.tags = clean_tags(value.split(',')),
                "icon" => match Icon::parse(value) {
                    Ok(icon) => m.icon = Some(icon),
                    Err(_) => m.other.push(line.to_string()),
                },
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
        // One line each: a line break or a '#' would end the value.
        let clean = |v: &str| {
            v.replace(['\n', '\r'], " ")
                .replace('#', "")
                .trim()
                .to_string()
        };
        for (key, value) in [
            ("description", &self.description),
            ("author", &self.author),
            ("version", &self.version),
            ("game", &self.game),
        ] {
            let value = clean(value);
            if !value.is_empty() {
                out.push_str(&format!("{key} = {value}\n"));
            }
        }
        if let Some(icon) = &self.icon {
            out.push_str(&format!("icon = {icon}\n"));
        }
        let tags = clean_tags(self.tags.iter().map(|t| clean(t)));
        if !tags.is_empty() {
            out.push_str(&format!("tags = {}\n", tags.join(", ")));
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

/// The largest species number an icon may name (Black and White's 649).
pub const ICON_SPECIES_MAX: u16 = 649;
/// The item numbers an icon may name: the ones the game's icon table holds.
pub const ICON_ITEMS: u16 = 627;
/// An icon file's limits: a square PNG of at most this many pixels a side
/// and this many bytes.
pub const ICON_FILE_SIDE: u32 = 256;
pub const ICON_FILE_BYTES: u64 = 64 * 1024;

/// A mod's picture: an item's bag icon or a species sprite from the
/// player's ROM, or a PNG in the mod folder.
///
/// ```text
/// icon = item 280        Destiny Knot
/// icon = species 25      Pikachu
/// icon = icon.png        a file in the mod folder
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Icon {
    Item(u16),
    Species(u16),
    File(String),
}

impl Icon {
    pub fn parse(text: &str) -> Result<Icon, String> {
        let text = text.trim();
        let number = |kind: &str, v: &str, max: u16| -> Result<u16, String> {
            v.trim()
                .parse::<u16>()
                .ok()
                .filter(|n| (1..=max).contains(n))
                .ok_or_else(|| format!("{kind} {} is not a number from 1 to {max}", v.trim()))
        };
        if let Some(v) = text.strip_prefix("item ") {
            return number("item", v, ICON_ITEMS - 1).map(Icon::Item);
        }
        if let Some(v) = text.strip_prefix("species ") {
            return number("species", v, ICON_SPECIES_MAX).map(Icon::Species);
        }
        let path = text.replace('\\', "/");
        if path.is_empty() {
            return Err("an icon is `item <n>`, `species <n>` or a .png file".into());
        }
        if !path.to_ascii_lowercase().ends_with(".png") {
            return Err(format!("{text}: an icon file is a .png"));
        }
        if path.starts_with('/')
            || path.contains(':')
            || path.split('/').any(|p| p == ".." || p.is_empty())
        {
            return Err(format!(
                "{text}: an icon file is a path inside the mod folder"
            ));
        }
        Ok(Icon::File(path))
    }
}

impl std::fmt::Display for Icon {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Icon::Item(n) => write!(f, "item {n}"),
            Icon::Species(n) => write!(f, "species {n}"),
            Icon::File(p) => write!(f, "{p}"),
        }
    }
}

/// Checks an icon file's bytes against the limits: a PNG, square, at most
/// 256x256, at most 64 KiB. Returns its side.
pub fn check_icon_png(bytes: &[u8]) -> Result<u32, String> {
    if bytes.len() as u64 > ICON_FILE_BYTES {
        return Err(format!(
            "the icon is {} bytes; at most {ICON_FILE_BYTES}",
            bytes.len()
        ));
    }
    // signature, then the IHDR chunk: length, "IHDR", width, height
    if bytes.len() < 24 || &bytes[..8] != b"\x89PNG\r\n\x1a\n" || &bytes[12..16] != b"IHDR" {
        return Err("the icon is not a PNG file".into());
    }
    let w = u32::from_be_bytes(bytes[16..20].try_into().unwrap());
    let h = u32::from_be_bytes(bytes[20..24].try_into().unwrap());
    if w != h || w == 0 || w > ICON_FILE_SIDE {
        return Err(format!(
            "the icon is {w}x{h}; it must be square, at most {ICON_FILE_SIDE}x{ICON_FILE_SIDE}"
        ));
    }
    Ok(w)
}

/// Tags as the manifest keeps them: trimmed, lowercased, no commas, no
/// empty or repeated ones, in the order given.
pub fn clean_tags<S: AsRef<str>>(tags: impl IntoIterator<Item = S>) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for t in tags {
        let t = t.as_ref().replace(',', " ").trim().to_lowercase();
        if !t.is_empty() && !out.contains(&t) {
            out.push(t);
        }
    }
    out
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
    fn description_author_version_game() {
        let mut m = Manifest::parse("name = A\n", "d");
        m.description = "Harder trainers.\nAnd #levels".into();
        m.author = "Someone".into();
        m.version = "0.2.0".into();
        m.game = "IRAF".into();
        let text = m.write();
        assert!(
            text.contains("description = Harder trainers. And levels\n"),
            "{text}"
        );
        let back = Manifest::parse(&text, "d");
        assert_eq!(back.author, "Someone");
        assert_eq!(back.version, "0.2.0");
        assert_eq!(back.game, "IRAF");
        assert!(!Manifest::parse("name = B", "d").write().contains("game"));
    }

    #[test]
    fn icons() {
        assert_eq!(Icon::parse("item 280"), Ok(Icon::Item(280)));
        assert_eq!(Icon::parse(" species 25 "), Ok(Icon::Species(25)));
        assert_eq!(
            Icon::parse("art\\icon.PNG"),
            Ok(Icon::File("art/icon.PNG".into()))
        );
        for bad in [
            "item 0",
            "item 700",
            "species 650",
            "species x",
            "icon.gif",
            "../x.png",
            "/x.png",
            "C:/x.png",
            "",
        ] {
            assert!(Icon::parse(bad).is_err(), "{bad}");
        }
        let m = Manifest::parse("name = A\nicon = item 216\n", "d");
        assert_eq!(m.icon, Some(Icon::Item(216)));
        assert!(m.write().contains("icon = item 216\n"));
        // a value that does not read is kept as written
        let bad = Manifest::parse("name = A\nicon = item 9999\n", "d");
        assert_eq!(bad.icon, None);
        assert!(bad.write().contains("icon = item 9999\n"));
        let mut png = b"\x89PNG\r\n\x1a\n\0\0\0\x0dIHDR".to_vec();
        png.extend(64u32.to_be_bytes());
        png.extend(64u32.to_be_bytes());
        assert_eq!(check_icon_png(&png), Ok(64));
        png[23] = 32;
        assert!(check_icon_png(&png).is_err());
        assert!(check_icon_png(b"GIF89a").is_err());
    }

    #[test]
    fn tags() {
        let m = Manifest::parse("name = A\ntags = Hard, rules,, hard , Nuzlocke\n", "d");
        assert_eq!(m.tags, vec!["hard", "rules", "nuzlocke"]);
        assert!(m.other.is_empty());
        let text = m.write();
        assert!(text.contains("tags = hard, rules, nuzlocke\n"), "{text}");
        let mut m = Manifest::parse("name = B", "d");
        assert!(!m.write().contains("tags"));
        m.tags = vec!["A,B".into(), "#x".into(), " ".into()];
        assert_eq!(Manifest::parse(&m.write(), "d").tags, vec!["a b", "x"]);
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
