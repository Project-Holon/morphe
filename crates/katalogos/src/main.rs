//! The `katalogos` tool: see the library (lib.rs) for what each command does.
//!
//!   katalogos pack <mod-dir> --repo <owner/name> --version <x.y.z> [--out <dir>]
//!   katalogos pack <mod-dir> --official <id> --version <x.y.z> [--out <dir>]
//!   katalogos check <index.json>
//!   katalogos pending <index.json>
//!   katalogos inspect <mod.json> <mod.bw> [icon.png]

use std::fs;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

use katalogos::{check, pack, pending, verify, ModJson, Origin};

fn arg(args: &[String], name: &str) -> Option<String> {
    args.iter()
        .position(|a| a == name)
        .and_then(|i| args.get(i + 1))
        .cloned()
}

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().collect();
    let r = match args.get(1).map(String::as_str) {
        Some("pack") if args.len() >= 3 => {
            let dir = PathBuf::from(&args[2]);
            let origin = match (arg(&args, "--official"), arg(&args, "--repo")) {
                (Some(id), None) => Some(Origin::Official(id)),
                (None, Some(r)) => Some(Origin::Repo(r)),
                _ => None,
            };
            let version = arg(&args, "--version").unwrap_or_default();
            let out = PathBuf::from(arg(&args, "--out").unwrap_or_else(|| "dist".into()));
            match origin {
                None => Err("pack takes one of --repo owner/name and --official <id>".into()),
                Some(o) => pack(&dir, &o, &version, &out).map(|j| {
                    println!("{} {}: {} files, {} bytes, sha256 {}", j.name, j.version, j.package.files, j.package.size, j.package.sha256);
                }),
            }
        }
        Some("check") if args.len() >= 3 => check(Path::new(&args[2])).map(|n| println!("all {n} mods ok")),
        Some("pending") if args.len() >= 3 => pending(Path::new(&args[2])).map(|list| {
            for (id, v) in list {
                println!("{id} {v}");
            }
        }),
        Some("inspect") if args.len() >= 4 => (|| {
            let j: ModJson = serde_json::from_slice(&fs::read(&args[2]).map_err(|e| e.to_string())?)
                .map_err(|e| format!("mod.json: {e}"))?;
            let bw = fs::read(&args[3]).map_err(|e| e.to_string())?;
            // the icon: the third file named, else icon.png beside mod.json
            let icon_path = args.get(4).map(PathBuf::from).unwrap_or_else(|| {
                Path::new(&args[2])
                    .with_file_name(katalogos::ICON_FILE)
            });
            let icon = fs::read(icon_path).ok();
            let tmp = std::env::temp_dir();
            verify(&j, &bw, icon.as_deref(), &tmp).map(|_| println!("ok {} {}", j.name, j.version))
        })(),
        _ => Err("usage: katalogos pack <mod-dir> (--repo owner/name | --official <id>) --version x.y.z [--out dir]\n       katalogos check <index.json>\n       katalogos pending <index.json>\n       katalogos inspect <mod.json> <mod.bw> [icon.png]".into()),
    };
    match r {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("katalogos: {e}");
            ExitCode::FAILURE
        }
    }
}
