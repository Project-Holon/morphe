//! Item icons (the bag's 32x32 pictures), drawn from the player's ROM.
//!
//! ```text
//! ARM9 table   per item a u16 tile member and a u16 palette member of
//!              a/0/2/5 (627 items); its address moves between cartridges
//! a/0/2/5      the icons' tiles and palettes; member 1 is the one-cell bank
//!              that lays out every icon (32x32)
//! ```
//!
//! The table's addresses are the ones kenon's `tools/code-addresses.py`
//! finds in each cartridge (kenon `core/src/addresses.json`, `item_icons`).

use crate::image::{render_cell, Rgba};
use crate::lz::decompress_if_compressed;
use crate::narc::Narc;
use crate::ncer::Ncer;
use crate::ncgr::Ncgr;
use crate::nclr::Palette;
use crate::rom::Rom;

const ICONS: &str = "a/0/2/5";
const LAYOUT: usize = 1;
/// Where the ARM9 image sits in memory.
const ARM9_ADDRESS: u32 = 0x0200_4000;
/// How many items the table holds.
pub const ITEMS: u16 = 627;

/// The item icon table's address in each cartridge, by game code.
const TABLE: &[(&str, u32)] = &[
    ("IRBO", 0x0209_EB6C),
    ("IRAO", 0x0209_EB84),
    ("IRBF", 0x0209_EAEC),
    ("IRAF", 0x0209_EB04),
    ("IRBD", 0x0209_EAEC),
    ("IRAD", 0x0209_EB04),
    ("IRBI", 0x0209_EAEC),
    ("IRAI", 0x0209_EB04),
    ("IRBS", 0x0209_EB3C),
    ("IRAS", 0x0209_EB54),
    ("IRBJ", 0x0209_EA14),
    ("IRAJ", 0x0209_EA2C),
];

/// The tile and palette members of an item's icon.
pub fn icon_members(rom: &Rom, item: u16) -> Result<(usize, usize), String> {
    if item >= ITEMS {
        return Err(format!("item {item} is past the table ({ITEMS} items)"));
    }
    let code = &rom.identity.game_code;
    let (_, at) = TABLE
        .iter()
        .find(|(c, _)| c == code)
        .ok_or_else(|| format!("no item icon table known for {code}"))?;
    let arm9 = rom.arm9().map_err(|e| e.to_string())?;
    let off = (at - ARM9_ADDRESS) as usize + item as usize * 4;
    let pair = arm9
        .get(off..off + 4)
        .ok_or("the item icon table is outside the ARM9 image")?;
    Ok((
        u16::from_le_bytes([pair[0], pair[1]]) as usize,
        u16::from_le_bytes([pair[2], pair[3]]) as usize,
    ))
}

/// An item's icon, as RGBA (32x32, transparent background).
pub fn item_icon(rom: &Rom, item: u16) -> Result<Rgba, String> {
    let (chr, pal) = icon_members(rom, item)?;
    let archive =
        Narc::parse(rom.file(ICONS).map_err(|e| e.to_string())?).map_err(|e| e.to_string())?;
    let member = |i: usize| {
        archive
            .raw(i)
            .map(|raw| decompress_if_compressed(raw).into_owned())
            .map_err(|e| e.to_string())
    };
    let tiles = Ncgr::parse(&member(chr)?).map_err(|e| e.to_string())?;
    let palette = Palette::parse(&member(pal)?).map_err(|e| e.to_string())?;
    let bank = Ncer::parse(&member(LAYOUT)?).map_err(|e| e.to_string())?;
    let cell = bank
        .cells
        .first()
        .ok_or("the item icon layout bank has no cell")?;
    Ok(render_cell(&bank, cell, &tiles, &palette, 0).image)
}
