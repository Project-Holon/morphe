//! Lists what a ROM changes against another: `romdiff <old.nds> <new.nds>`.

use nds::rom::Rom;
use nds::romdiff::{diff, FileDiff};

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.len() != 2 {
        eprintln!("usage: romdiff <old.nds> <new.nds>");
        std::process::exit(2);
    }
    let old = Rom::open(args[0].as_ref()).expect("open the old ROM");
    let new = Rom::open(args[1].as_ref()).expect("open the new ROM");
    println!("old {:?}\nnew {:?}", old.identity, new.identity);
    let d = diff(&old, &new).expect("compare");
    let mut members = 0;
    for f in &d.files {
        match f {
            FileDiff::Narc {
                path,
                changed,
                added,
                removed,
            } => {
                members += changed.len() + added.len();
                let shown: Vec<String> = changed.iter().take(12).map(|i| i.to_string()).collect();
                println!(
                    "narc  {path}: {} changed [{}{}], {} added, {} removed",
                    changed.len(),
                    shown.join(" "),
                    if changed.len() > 12 { " ..." } else { "" },
                    added.len(),
                    removed
                );
            }
            FileDiff::Whole {
                path,
                old_size,
                new_size,
            } => println!("file  {path}: {old_size} -> {new_size} bytes"),
        }
    }
    println!(
        "{} files changed ({members} members), {} added, {} removed",
        d.files.len(),
        d.added_files.len(),
        d.removed_files.len()
    );
    for p in d.added_files.iter().chain(&d.removed_files) {
        println!("  added/removed: {p}");
    }
    for c in &d.code {
        let bytes: u32 = c.ranges.iter().map(|(s, e)| e - s).sum();
        println!(
            "code  {} at {:#x}: {} ranges, {bytes} bytes, size {} -> {}",
            c.part,
            c.address,
            c.ranges.len(),
            c.old_size,
            c.new_size
        );
        for (s, e) in c.ranges.iter().take(40) {
            println!("        {s:#010x}..{e:#010x} ({} bytes)", e - s);
        }
    }
    println!("header ranges {:x?}", d.header);
    println!("banner changed: {}", d.banner_changed);
}
