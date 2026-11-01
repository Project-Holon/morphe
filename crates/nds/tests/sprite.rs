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
        let rom = Rom::open(&path).unwrap();
        for g in [nds::sprite::PLAYER_MALE, nds::sprite::PLAYER_FEMALE] {
            let t = nds::sprite::trainer_front(&rom, g).unwrap();
            assert_eq!((t.width, t.height), (80, 80), "{file}");
            assert!(t.pixels.chunks(4).any(|p| p[3] > 0), "{file}: player {g}");
        }
    }
}

/// The badge case from real ROMs: eight 32x64 badges and outlines, each
/// with something drawn. With `NDS_BADGES_OUT` set, writes them as PNG.
#[test]
fn badge_case() {
    let Some(dir) = std::env::var_os("NDS_ROMS").map(std::path::PathBuf::from) else {
        return;
    };
    for entry in std::fs::read_dir(&dir).unwrap().flatten() {
        let path = entry.path();
        if path.extension().is_none_or(|x| x != "nds") {
            continue;
        }
        let art = nds::badges::badge_art(&Rom::open(&path).unwrap()).unwrap();
        for (i, (b, o)) in art.badges.iter().zip(&art.outlines).enumerate() {
            assert!(
                b.pixels.chunks(4).any(|p| p[3] > 0),
                "{}: badge {i}",
                path.display()
            );
            assert!(
                o.pixels.chunks(4).any(|p| p[3] > 0),
                "{}: outline {i}",
                path.display()
            );
            if let Some(out) = std::env::var_os("NDS_BADGES_OUT") {
                let out = std::path::PathBuf::from(out);
                let stem = path
                    .file_stem()
                    .unwrap()
                    .to_string_lossy()
                    .replace(' ', "_");
                std::fs::write(
                    out.join(format!("{stem}_b{i}.png")),
                    nds::image::encode_png(b),
                )
                .unwrap();
                std::fs::write(
                    out.join(format!("{stem}_o{i}.png")),
                    nds::image::encode_png(o),
                )
                .unwrap();
            }
        }
    }
}
