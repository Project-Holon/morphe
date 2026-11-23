//! What one ROM changes against another: the NitroFS files (per member for a
//! NARC archive), the code (ARM9, the overlays, ARM7), the header and the
//! banner. It is the first step in turning a patched ROM (a ROM hack) into a
//! mod: changed members and files become overrides, and changed code is
//! listed, because a data mod cannot carry it.

use std::collections::BTreeSet;

use serde::Serialize;

use crate::narc::Narc;
use crate::rom::{Error, Rom};

/// Changed bytes closer than this are reported as one range.
const MERGE_GAP: usize = 8;

#[derive(Debug, Clone, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RomDiff {
    pub files: Vec<FileDiff>,
    /// Paths only the new ROM has.
    pub added_files: Vec<String>,
    /// Paths only the old ROM has.
    pub removed_files: Vec<String>,
    pub code: Vec<CodeDiff>,
    /// Changed byte ranges of the header (offsets). Moved files change the
    /// table offsets here, so this is information, not a change to carry.
    pub header: Vec<(usize, usize)>,
    pub banner_changed: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase", tag = "kind")]
pub enum FileDiff {
    /// A NARC archive: the members that differ, the ones past the old end,
    /// and how many the new one lacks at its end.
    Narc {
        path: String,
        changed: Vec<usize>,
        added: Vec<usize>,
        removed: usize,
    },
    /// Any other file, or a NARC that no longer parses: the whole file.
    Whole {
        path: String,
        old_size: usize,
        new_size: usize,
    },
}

impl FileDiff {
    pub fn path(&self) -> &str {
        match self {
            FileDiff::Narc { path, .. } | FileDiff::Whole { path, .. } => path,
        }
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CodeDiff {
    /// "arm9", "overlay 93", "arm7".
    pub part: String,
    pub overlay: Option<u16>,
    /// Where the part is loaded.
    pub address: u32,
    /// Changed ranges as load addresses, end exclusive.
    pub ranges: Vec<(u32, u32)>,
    pub old_size: usize,
    pub new_size: usize,
}

/// Compares two ROMs.
pub fn diff(old: &Rom, new: &Rom) -> Result<RomDiff, Error> {
    let mut d = RomDiff::default();

    let old_paths: BTreeSet<&str> = old.paths().collect();
    let new_paths: BTreeSet<&str> = new.paths().collect();
    d.added_files = new_paths
        .difference(&old_paths)
        .map(|s| s.to_string())
        .collect();
    d.removed_files = old_paths
        .difference(&new_paths)
        .map(|s| s.to_string())
        .collect();
    for path in old_paths.intersection(&new_paths) {
        let (a, b) = (old.file(path)?, new.file(path)?);
        if a == b {
            continue;
        }
        d.files.push(file_diff(path, a, b));
    }

    let arm9_at = old.arm9_address()?;
    push_code(
        &mut d.code,
        "arm9".into(),
        None,
        arm9_at,
        &old.arm9()?,
        &new.arm9()?,
    );
    let count = old.overlays().len().max(new.overlays().len());
    for n in 0..count as u16 {
        let a = old
            .overlays()
            .get(n as usize)
            .map(|_| old.overlay(n))
            .transpose()?;
        let b = new
            .overlays()
            .get(n as usize)
            .map(|_| new.overlay(n))
            .transpose()?;
        let address = new
            .overlays()
            .get(n as usize)
            .or(old.overlays().get(n as usize))
            .map_or(0, |e| e.address);
        push_code(
            &mut d.code,
            format!("overlay {n}"),
            Some(n),
            address,
            a.as_deref().unwrap_or(&[]),
            b.as_deref().unwrap_or(&[]),
        );
    }
    let (arm7_at, arm7_old) = old.arm7()?;
    push_code(
        &mut d.code,
        "arm7".into(),
        None,
        arm7_at,
        &arm7_old,
        &new.arm7()?.1,
    );

    d.header = ranges(&old.header()?, &new.header()?);
    d.banner_changed = old.banner()? != new.banner()?;
    Ok(d)
}

fn file_diff(path: &str, a: Vec<u8>, b: Vec<u8>) -> FileDiff {
    let (old_size, new_size) = (a.len(), b.len());
    if a.starts_with(b"NARC") && b.starts_with(b"NARC") {
        if let (Ok(na), Ok(nb)) = (Narc::parse(a), Narc::parse(b)) {
            let common = na.len().min(nb.len());
            let changed = (0..common)
                .filter(|&i| na.raw(i).ok() != nb.raw(i).ok())
                .collect();
            return FileDiff::Narc {
                path: path.to_string(),
                changed,
                added: (common..nb.len()).collect(),
                removed: na.len().saturating_sub(nb.len()),
            };
        }
    }
    FileDiff::Whole {
        path: path.to_string(),
        old_size,
        new_size,
    }
}

fn push_code(
    out: &mut Vec<CodeDiff>,
    part: String,
    overlay: Option<u16>,
    address: u32,
    a: &[u8],
    b: &[u8],
) {
    if a == b {
        return;
    }
    let ranges = ranges(a, b)
        .into_iter()
        .map(|(s, e)| {
            (
                address.wrapping_add(s as u32),
                address.wrapping_add(e as u32),
            )
        })
        .collect();
    out.push(CodeDiff {
        part,
        overlay,
        address,
        ranges,
        old_size: a.len(),
        new_size: b.len(),
    });
}

/// The byte ranges that differ (end exclusive); bytes past the shorter
/// buffer count as changed.
pub fn ranges(a: &[u8], b: &[u8]) -> Vec<(usize, usize)> {
    let n = a.len().max(b.len());
    let mut out: Vec<(usize, usize)> = Vec::new();
    let mut i = 0;
    while i < n {
        if a.get(i) == b.get(i) {
            i += 1;
            continue;
        }
        let start = i;
        while i < n && a.get(i) != b.get(i) {
            i += 1;
        }
        match out.last_mut() {
            Some(last) if start - last.1 < MERGE_GAP => last.1 = i,
            _ => out.push((start, i)),
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ranges_merge_close_changes() {
        assert_eq!(ranges(b"abcdef", b"abcdef"), vec![]);
        assert_eq!(
            ranges(b"abcdefghijklmnop", b"aXcdefghijklmnoX"),
            vec![(1, 2), (15, 16)]
        );
        assert_eq!(ranges(b"abcdef", b"aXcXef"), vec![(1, 4)]);
        assert_eq!(ranges(b"abc", b"abcde"), vec![(3, 5)]);
    }
}
