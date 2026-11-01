//! Draws the box legendaries from real ROMs. Set `NDS_ROMS` to a folder
//! holding black.nds and white.nds (the names aletheia's docs/ROMS.md
//! uses); without it the test does nothing.

use nds::rom::Rom;
use nds::sprite::{battle_front, RESHIRAM, ZEKROM};

#[test]
fn box_legendaries() {
    let Some(dir) = std::env::var_os("NDS_ROMS").map(std::path::PathBuf::from) else {
        eprintln!("NDS_ROMS not set; skipped");
        return;
    };
    for (file, species) in [("black.nds", ZEKROM), ("white.nds", RESHIRAM)] {
        let path = dir.join(file);
        if !path.is_file() {
            continue;
        }
        let img = battle_front(&Rom::open(&path).unwrap(), species).unwrap();
        assert_eq!((img.width, img.height), (96, 96), "{file}");
        assert!(img.pixels.chunks(4).any(|p| p[3] > 0), "{file}: empty");
    }
}
