//! Nintendo DS cartridges and their 2D formats: the ROM image and NitroFS,
//! NARC archives, LZ and BLZ compression, palettes (NCLR), characters
//! (NCGR), cells (NCER), screens (NSCR), and drawing them to RGBA and PNG.
//! Shared by kenon and the eidos launcher. Reads the player's own ROM; holds
//! no game data.

pub mod blz;
pub mod image;
pub mod lz;
pub mod narc;
pub mod ncer;
pub mod ncgr;
pub mod nclr;
pub mod nitro;
pub mod nscr;
pub mod rom;
pub mod sprite;
