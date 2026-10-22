//! Mod folders for the native port: everything that can be read, written and
//! checked without the game's data.
//!
//! - [`manifest`]: `eidos_mod.txt` and the editor's notes.
//! - [`modcode`]: settings, hook lines, rule files, tables, the rule checker
//!   and conflicts between mods.
//! - [`modopts`]: the options grammar.
//! - [`package`]: `.bw` packages, packed and unpacked with safety limits.
//! - [`sdk`]: the code-mod header, contract and module check.
//!
//! No Tauri, no ROM code.

pub mod manifest;
pub mod modcode;
pub mod modopts;
pub mod package;
pub mod sdk;
