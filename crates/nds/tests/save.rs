//! Reads real saves and place names. Set `NDS_SAVES` to a folder of .sav
//! files and `NDS_ROMS` to a folder of ROMs; without them the tests do
//! nothing. Run with `--nocapture` to see what each file holds.

use std::path::PathBuf;

use nds::place::place_names;
use nds::rom::Rom;
use nds::save::read;

fn dir(var: &str) -> Option<PathBuf> {
    std::env::var_os(var).map(PathBuf::from)
}

fn files(dir: &std::path::Path, ext: &str) -> Vec<PathBuf> {
    let mut out = Vec::new();
    let mut todo = vec![dir.to_path_buf()];
    while let Some(d) = todo.pop() {
        for e in std::fs::read_dir(&d).into_iter().flatten().flatten() {
            let p = e.path();
            if p.is_dir() {
                todo.push(p);
            } else if p.extension().is_some_and(|x| x == ext) {
                out.push(p);
            }
        }
    }
    out.sort();
    out
}

#[test]
fn real_saves() {
    let Some(saves) = dir("NDS_SAVES") else {
        eprintln!("NDS_SAVES not set; skipped");
        return;
    };
    for path in files(&saves, "sav") {
        match read(&std::fs::read(&path).unwrap()) {
            Ok(Some(s)) => {
                assert!(!s.trainer.is_empty(), "{}", path.display());
                assert!(s.party.len() <= 6);
                eprintln!("{}: {s:?}", path.display());
            }
            Ok(None) => eprintln!("{}: no save", path.display()),
            Err(e) => panic!("{}: {e}", path.display()),
        }
    }
}

#[test]
fn real_place_names() {
    let Some(roms) = dir("NDS_ROMS") else {
        eprintln!("NDS_ROMS not set; skipped");
        return;
    };
    for path in files(&roms, "nds") {
        let Ok(rom) = Rom::open(&path) else { continue };
        let names = place_names(&rom).unwrap();
        assert!(
            names.len() > 400,
            "{}: {} zones",
            path.display(),
            names.len()
        );
        // zones real saves stood in, for the eye
        eprintln!(
            "{}: {} zones; 389={:?} 391={:?} 397={:?}",
            path.display(),
            names.len(),
            names[389],
            names[391],
            names[397]
        );
    }
}
