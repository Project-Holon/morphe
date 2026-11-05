//! Nintendo DS cartridges and their 2D formats: the ROM image and NitroFS,
//! NARC archives, LZ and BLZ compression, palettes (NCLR), characters
//! (NCGR), cells (NCER), screens (NSCR), and drawing them to RGBA and PNG;
//! the game's text, its place names, the badges, and Black and White save
//! files.
//! Shared by kenon and the eidos launcher. Reads the player's own ROM; holds
//! no game data.

pub mod badges;
pub mod blz;
pub mod image;
pub mod item;
pub mod lz;
pub mod msg;
pub mod narc;
pub mod ncer;
pub mod ncgr;
pub mod nclr;
pub mod nitro;
pub mod nscr;
pub mod place;
pub mod rom;
pub mod save;
pub mod sprite;
