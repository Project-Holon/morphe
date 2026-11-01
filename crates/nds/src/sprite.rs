//! Pokemon battle sprites, drawn from the player's ROM.

use crate::image::{encode_png, render_cell, Rgba};
use crate::narc::Narc;
use crate::ncer::Ncer;
use crate::ncgr::Ncgr;
use crate::nclr::Palette;
use crate::rom::Rom;

/// The battle sprite archive: 20 members per species; the front sheet at
/// +0, the palette at +18. Member 4 of species 0 is a one-cell bank that
/// lays out the whole 96x96 picture for every species.
const BATTLE_SPRITES: &str = "a/0/0/4";
const MEMBERS: usize = 20;
const LAYOUT_BANK: usize = 4;

/// Zekrom, the legendary on Pokemon Black's box.
pub const ZEKROM: u16 = 644;
/// Reshiram, the legendary on Pokemon White's box.
pub const RESHIRAM: u16 = 643;

/// The front battle sprite of a species, as RGBA (transparent background).
pub fn battle_front(rom: &Rom, species: u16) -> Result<Rgba, String> {
    let archive = Narc::parse(rom.file(BATTLE_SPRITES).map_err(|e| e.to_string())?)
        .map_err(|e| e.to_string())?;
    let block = species as usize * MEMBERS;
    // Some members are LZ compressed, some are not.
    let member = |i: usize| {
        archive
            .raw(i)
            .map(|raw| crate::lz::decompress_if_compressed(raw).into_owned())
            .map_err(|e| e.to_string())
    };
    let tiles = Ncgr::parse(&member(block)?).map_err(|e| e.to_string())?;
    let palette = Palette::parse(&member(block + 18)?).map_err(|e| e.to_string())?;
    let bank = Ncer::parse(&member(LAYOUT_BANK)?).map_err(|e| e.to_string())?;
    let cell = bank.cells.first().ok_or("the layout bank has no cell")?;
    Ok(render_cell(&bank, cell, &tiles, &palette, 0).image)
}

/// The front battle sprite of a species, as PNG.
pub fn battle_front_png(rom: &Rom, species: u16) -> Result<Vec<u8>, String> {
    battle_front(rom, species).map(|img| encode_png(&img))
}
