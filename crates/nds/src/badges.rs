//! The eight Unova badges and their empty slots, as the trainer card's badge
//! case draws them (aletheia `src/game/ov111/viewer_text.c`), from the
//! player's ROM.
//!
//! ```text
//! a/0/4/0   member 27  OBJ characters   (CLACT_RegisterCharacter 0x1B)
//!           member 2   OBJ palette      (CLACT_RegisterPalette 2)
//!           member 45  cells, one per badge, 32x64 (CLACT_RegisterCellAnim 0x2D)
//!           member 25  BG characters    (ARC_UTIL_UploadBgChar 0x19)
//!           member 1   BG palette       (ARC_UTIL_UploadPaletteWhole 1)
//!           member 33  BG screen: the case's strip, eight badge-shaped
//!                      slots 32 pixels apart, rows 88..168
//! ```
//!
//! Cell `i` is badge `i` in the order the game awards them (bit `i` of the
//! save's badge word). The outline of slot `i` is the slot's dark silhouette
//! cut from the strip, the case's frame left out.

use crate::image::{render_cell, render_screen, Rgba, Zero};
use crate::lz::decompress_if_compressed;
use crate::narc::Narc;
use crate::ncer::Ncer;
use crate::ncgr::Ncgr;
use crate::nclr::Palette;
use crate::nscr::Nscr;
use crate::rom::Rom;

const CASE: &str = "a/0/4/0";
const OBJ_CHARS: usize = 27;
const OBJ_PALETTE: usize = 2;
const CELLS: usize = 45;
const BG_CHARS: usize = 25;
const BG_PALETTE: usize = 1;
const STRIP: usize = 33;

/// Width and height of one badge and one slot.
pub const WIDTH: usize = 32;
pub const HEIGHT: usize = 64;
/// The strip's slots: the first row of the 64-pixel band they sit in.
const SLOT_TOP: usize = 96;

pub struct BadgeArt {
    /// The badges, in award order, 32x64 RGBA.
    pub badges: Vec<Rgba>,
    /// The empty slot of each badge, 32x64 RGBA.
    pub outlines: Vec<Rgba>,
}

fn member(archive: &Narc, i: usize) -> Result<Vec<u8>, String> {
    archive
        .raw(i)
        .map(|raw| decompress_if_compressed(raw).into_owned())
        .map_err(|e| e.to_string())
}

/// The case's frame colour: the strip's most common opaque colour.
fn frame_colour(img: &Rgba) -> [u8; 4] {
    let mut counts = std::collections::HashMap::new();
    for &p in img.pixels.as_chunks::<4>().0 {
        if p[3] != 0 {
            *counts.entry(p).or_insert(0usize) += 1;
        }
    }
    counts
        .into_iter()
        .max_by_key(|&(_, n)| n)
        .map(|(c, _)| c)
        .unwrap_or([0; 4])
}

/// The silhouette of slot `i`: the pixels darker than the frame, cut from
/// the strip, with the case's border and divider lines (the dark pixels
/// that touch the slot's edges) left out.
fn outline(strip: &Rgba, i: usize, frame: [u8; 4]) -> Rgba {
    let mut out = Rgba::new(WIDTH, HEIGHT);
    let luma = |p: [u8; 4]| p[0] as u32 + p[1] as u32 + p[2] as u32;
    let dark = luma(frame);
    for y in 0..HEIGHT {
        for x in 1..WIDTH - 1 {
            let p = strip.get(i * WIDTH + x, SLOT_TOP + y);
            if p[3] != 0 && luma(p) < dark {
                let d = (y * WIDTH + x) * 4;
                out.pixels[d..d + 4].copy_from_slice(&p);
            }
        }
    }
    out
}

/// The eight badges and their outlines.
pub fn badge_art(rom: &Rom) -> Result<BadgeArt, String> {
    let archive =
        Narc::parse(rom.file(CASE).map_err(|e| e.to_string())?).map_err(|e| e.to_string())?;
    let tiles = Ncgr::parse(&member(&archive, OBJ_CHARS)?).map_err(|e| e.to_string())?;
    let palette = Palette::parse(&member(&archive, OBJ_PALETTE)?).map_err(|e| e.to_string())?;
    let bank = Ncer::parse(&member(&archive, CELLS)?).map_err(|e| e.to_string())?;
    let badges: Vec<Rgba> = bank
        .cells
        .iter()
        .take(8)
        .map(|c| render_cell(&bank, c, &tiles, &palette, 0).image)
        .collect();
    if badges.len() != 8
        || badges
            .iter()
            .any(|b| (b.width, b.height) != (WIDTH, HEIGHT))
    {
        return Err("the badge case is not laid out as Black and White's".into());
    }
    let bg_tiles = Ncgr::parse(&member(&archive, BG_CHARS)?).map_err(|e| e.to_string())?;
    let bg_palette = Palette::parse(&member(&archive, BG_PALETTE)?).map_err(|e| e.to_string())?;
    let screen = Nscr::parse(&member(&archive, STRIP)?).map_err(|e| e.to_string())?;
    let strip = render_screen(&screen, &bg_tiles, &bg_palette, Zero::Transparent);
    if strip.width < 8 * WIDTH || strip.height < SLOT_TOP + HEIGHT {
        return Err("the badge case strip is smaller than Black and White's".into());
    }
    let frame = frame_colour(&strip);
    let outlines = (0..8).map(|i| outline(&strip, i, frame)).collect();
    Ok(BadgeArt { badges, outlines })
}
