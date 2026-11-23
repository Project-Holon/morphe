//! Applies an xdelta3 patch: `xdelta_apply <source> <patch> [<output>]`.
//! Without an output it only checks the patch against the source.

use std::time::Instant;

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.len() < 2 {
        eprintln!("usage: xdelta_apply <source> <patch> [<output>]");
        std::process::exit(2);
    }
    let source = std::fs::read(&args[0]).expect("read the source");
    let patch = std::fs::read(&args[1]).expect("read the patch");
    let info = xdelta::info(&patch).unwrap_or_else(|e| {
        eprintln!("{e}");
        std::process::exit(1)
    });
    println!(
        "windows {}, output {} bytes, source read up to {}, checksums {}, secondary {:?}",
        info.windows, info.target_size, info.source_needed, info.checksummed, info.secondary
    );
    if let Some(app) = &info.app_header {
        println!("app header: {}", String::from_utf8_lossy(app));
    }
    let t = Instant::now();
    match xdelta::apply_checked(&source, &patch) {
        Ok((out, bad)) => {
            println!("applied in {:.2?}: {} bytes", t.elapsed(), out.len());
            for m in &bad {
                let piece = &out[m.offset as usize..(m.offset + m.len) as usize];
                let mut seen = [false; 256];
                piece.iter().for_each(|&b| seen[b as usize] = true);
                let distinct = seen.iter().filter(|&&s| s).count();
                let same_as_source =
                    source.get(m.offset as usize..(m.offset + m.len) as usize) == Some(piece);
                println!(
                    "window {} at {:#x}+{:#x} fails its checksum: {} distinct byte values, same as the source there: {}",
                    m.window, m.offset, m.len, distinct, same_as_source
                );
            }
            if let Some(path) = args.get(2) {
                std::fs::write(path, &out).expect("write the output");
            }
        }
        Err(e) => {
            eprintln!("{e}");
            std::process::exit(1);
        }
    }
}
