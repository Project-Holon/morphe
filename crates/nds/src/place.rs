//! Place names of Pokemon Black and White zones, from the player's ROM.
//!
//! ```text
//! zone headers  a/0/1/2 member 0: 48-byte records, record = zone id;
//!               place name id in bits 0-9 of the u16 at 0x1A
//! place names   a/0/0/2 member 89, one message per place name id
//! ```
//!
//! The same layouts kenon's world view reads (docs/game-data-map.md).

use crate::lz::decompress_if_compressed;
use crate::msg::{plain, Bank};
use crate::narc::Narc;
use crate::rom::Rom;

const ZONE_HEADERS: &str = "a/0/1/2";
const ZONE_RECORD: usize = 48;
const TEXT: &str = "a/0/0/2";
const PLACE_NAMES: usize = 89;

fn member(rom: &Rom, path: &str, index: usize) -> Result<Vec<u8>, String> {
    let archive =
        Narc::parse(rom.file(path).map_err(|e| e.to_string())?).map_err(|e| e.to_string())?;
    let raw = archive.raw(index).map_err(|e| e.to_string())?;
    Ok(decompress_if_compressed(raw).into_owned())
}

/// The place name of every zone, indexed by zone id ("" when the zone has
/// none). Control codes in a name are left out.
pub fn place_names(rom: &Rom) -> Result<Vec<String>, String> {
    let headers = member(rom, ZONE_HEADERS, 0)?;
    let bank = Bank::parse(&member(rom, TEXT, PLACE_NAMES)?).map_err(|e| e.to_string())?;
    let names: Vec<String> = bank
        .messages()
        .iter()
        .map(|m| {
            let units: Vec<u16> = crate::msg::tokens(&m.units)
                .into_iter()
                .filter_map(|t| match t {
                    crate::msg::Token::Char(c) => Some(c),
                    _ => None,
                })
                .collect();
            plain(&units, None).trim().to_string()
        })
        .collect();
    Ok(headers
        .as_chunks::<ZONE_RECORD>()
        .0
        .iter()
        .map(|r| {
            let id = (u16::from_le_bytes([r[0x1A], r[0x1B]]) & 0x3FF) as usize;
            names.get(id).cloned().unwrap_or_default()
        })
        .collect())
}
