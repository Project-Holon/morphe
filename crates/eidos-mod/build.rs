// Embeds the mod C library (src/modsdk/v7/libc) as LIBC_FILES: each file
// with the path it takes in a mod project. kenon copies them when a mod asks
// for the library.
use std::fs;
use std::path::{Path, PathBuf};

fn walk(dir: &Path, out: &mut Vec<PathBuf>) {
    let mut entries: Vec<_> = fs::read_dir(dir)
        .expect("libc folder")
        .filter_map(Result::ok)
        .map(|e| e.path())
        .collect();
    entries.sort();
    for p in entries {
        if p.is_dir() {
            walk(&p, out);
        } else {
            out.push(p);
        }
    }
}

fn main() {
    let root = PathBuf::from(std::env::var("CARGO_MANIFEST_DIR").unwrap());
    let libc = root.join("src/modsdk/v7/libc");
    println!("cargo:rerun-if-changed={}", libc.display());
    let mut files = Vec::new();
    walk(&libc, &mut files);
    let mut code = String::from("/// The mod C library: (path in a mod project, contents).\npub const LIBC_FILES: &[(&str, &str)] = &[\n");
    for f in &files {
        let rel = f
            .strip_prefix(&libc)
            .unwrap()
            .to_string_lossy()
            .replace('\\', "/");
        let dest = match rel.split_once('/') {
            Some(("include", rest)) => format!("include/{rest}"),
            Some(("src", rest)) => format!("src/libc/{rest}"),
            Some(("licenses", rest)) => format!("src/libc/licenses/{rest}"),
            _ => format!("src/libc/{rel}"),
        };
        code.push_str(&format!(
            "    ({dest:?}, include_str!({:?})),\n",
            f.display().to_string()
        ));
    }
    code.push_str("];\n");
    let out = PathBuf::from(std::env::var("OUT_DIR").unwrap()).join("libc_files.rs");
    fs::write(out, code).unwrap();
}
