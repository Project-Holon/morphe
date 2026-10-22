//! The code-mod SDK as eidos publishes it: the header, the contract file and
//! the starting sources (`modsdk/v5/`), the header reader that feeds
//! completion, and the check of a built WebAssembly module against the
//! contract. No compiler is needed for any of it.
//!
//! The header is eidos's published `sdk/api/v5/eidos_wasm.h`, copied
//! unchanged; when eidos changes the API, replace it and nothing else here
//! changes.

use std::fs;
use std::path::Path;

use serde::Serialize;

pub const HEADER_NAME: &str = "eidos_wasm.h";
pub const HEADER: &str = include_str!("modsdk/v5/eidos_wasm.h");
pub const TEMPLATE_MOD: &str = include_str!("modsdk/v5/mod.c");
pub const TEMPLATE_ALLOC: &str = include_str!("modsdk/v5/alloc.c");
pub const CONTRACT: &str = include_str!("modsdk/v5/contract.txt");

/// Where the sources live in a mod folder.
pub const SRC_DIR: &str = "src";
pub const INCLUDE_DIR: &str = "include";

// ---------------------------------------------------------------------
// The contract
// ---------------------------------------------------------------------

/// `modsdk/v5/contract.txt`, read.
#[derive(Debug, Clone, Default)]
pub struct Contract {
    pub api: u32,
    pub target: String,
    pub import_module: String,
    pub output: String,
    pub flags: Vec<String>,
    /// Required function exports with their result type (`i32` or none).
    pub export_funcs: Vec<(String, Option<String>)>,
    pub export_memory: String,
    pub export_table: bool,
}

impl Contract {
    pub fn parse(text: &str) -> Self {
        let mut c = Contract::default();
        for line in text.lines() {
            let line = line.split('#').next().unwrap_or("").trim();
            let Some((key, value)) = line.split_once('=') else {
                continue;
            };
            let value = value.trim();
            match key.trim() {
                "api" => c.api = value.parse().unwrap_or(0),
                "target" => c.target = value.to_string(),
                "import_module" => c.import_module = value.to_string(),
                "output" => c.output = value.to_string(),
                "flag" => c.flags.push(value.to_string()),
                "export_func" => {
                    let (name, result) = value.split_once(':').unwrap_or((value, "none"));
                    let result = (result.trim() != "none").then(|| result.trim().to_string());
                    c.export_funcs.push((name.trim().to_string(), result));
                }
                "export_memory" => c.export_memory = value.to_string(),
                "export_table" => c.export_table = value == "yes",
                _ => {}
            }
        }
        c
    }
}

/// The contract the build follows.
pub fn contract() -> Contract {
    Contract::parse(CONTRACT)
}

// ---------------------------------------------------------------------
// The header, for completion
// ---------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ApiFunction {
    pub name: String,
    /// The declaration without the import macro.
    pub signature: String,
    pub doc: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ApiConstant {
    pub name: String,
    pub value: String,
    /// The comment above its group (`battle`, `session`).
    pub group: String,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CodeApi {
    pub functions: Vec<ApiFunction>,
    pub constants: Vec<ApiConstant>,
}

/// Reads the function declarations and the enum constants out of a header.
pub fn parse_header(text: &str) -> CodeApi {
    let mut api = CodeApi::default();
    let mut doc: Vec<String> = Vec::new();
    let mut in_comment = false;
    let mut group = String::new();
    // A declaration that wraps onto more lines is joined into one.
    let mut joined = String::new();
    for raw in text.lines() {
        let mut trimmed = raw.trim();
        // A declaration may carry its note after it: `int f(void); /* note */`.
        let mut note = None;
        if !in_comment && !trimmed.starts_with("/*") {
            if let Some(at) = trimmed.find("/*").filter(|_| trimmed.ends_with("*/")) {
                let body = trimmed[at..]
                    .trim_start_matches("/*")
                    .trim_end_matches("*/");
                note = Some(body.trim().to_string());
                trimmed = trimmed[..at].trim_end();
            }
        }
        let whole;
        let line: &str = if !joined.is_empty()
            || (trimmed.starts_with("EIDOS_IMPORT(") && !trimmed.ends_with(';'))
        {
            if !joined.is_empty() {
                joined.push(' ');
            }
            joined.push_str(trimmed);
            if !trimmed.ends_with(';') {
                continue;
            }
            whole = std::mem::take(&mut joined);
            &whole
        } else {
            trimmed
        };
        if in_comment || line.starts_with("/*") {
            let body = line
                .trim_start_matches("/*")
                .trim_start_matches('*')
                .trim_end_matches("*/")
                .trim();
            if !in_comment {
                doc.clear();
            }
            if !body.is_empty() {
                doc.push(body.to_string());
            }
            in_comment = !line.ends_with("*/");
            if !in_comment && doc.len() == 1 {
                group = doc[0].clone();
            }
            continue;
        }
        if let Some((name, value)) = line.split_once('=') {
            let name = name.trim();
            let value = value.split("/*").next().unwrap_or("").trim();
            let value = value.trim_end_matches(',').trim();
            if name.starts_with("EIDOS_")
                && name
                    .bytes()
                    .all(|b| b.is_ascii_uppercase() || b.is_ascii_digit() || b == b'_')
                && !value.is_empty()
            {
                api.constants.push(ApiConstant {
                    name: name.to_string(),
                    value: value.to_string(),
                    group: group.clone(),
                });
                continue;
            }
        }
        if let Some(decl) = declaration(line) {
            api.functions.push(ApiFunction {
                name: decl.0,
                signature: decl.1,
                doc: note.unwrap_or_else(|| doc.join(" ")),
            });
        }
        if !line.is_empty() {
            doc.clear();
        }
    }
    api
}

/// `(name, signature)` of a function declaration line, if it is one.
fn declaration(line: &str) -> Option<(String, String)> {
    if line.starts_with('#') || line.starts_with("typedef") || !line.ends_with(");") {
        return None;
    }
    let mut sig = line.trim_end_matches(';').trim().to_string();
    // Drop the import macro: EIDOS_IMPORT("log") void eidos_log(...)
    if let Some(at) = sig.find("EIDOS_IMPORT(") {
        let close = sig[at..].find(')')? + at + 1;
        sig = sig[close..].trim().to_string();
    }
    let open = sig.find('(')?;
    let head = sig[..open].trim_end();
    let name = head
        .rsplit(|c: char| !(c.is_ascii_alphanumeric() || c == '_'))
        .next()?
        .to_string();
    (!name.is_empty()).then_some((name, sig))
}

/// The header of the project (its own copy), else the template's.
pub fn code_api(root: &Path) -> CodeApi {
    let own = fs::read_to_string(root.join(INCLUDE_DIR).join(HEADER_NAME)).ok();
    parse_header(own.as_deref().unwrap_or(HEADER))
}

// ---------------------------------------------------------------------
// The module check
// ---------------------------------------------------------------------

/// What the check reads from a module.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ModuleInfo {
    pub imports: Vec<Import>,
    pub exports: Vec<Export>,
    /// Parameter and result types of each type entry.
    types: Vec<(Vec<u8>, Vec<u8>)>,
    /// The type entry of every function, imported ones first.
    func_types: Vec<u32>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Import {
    pub module: String,
    pub name: String,
    pub kind: u8,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Export {
    pub name: String,
    pub kind: u8,
    pub index: u32,
}

const KIND_FUNC: u8 = 0;
const KIND_TABLE: u8 = 1;
const KIND_MEMORY: u8 = 2;
const KIND_GLOBAL: u8 = 3;

struct Bytes<'a> {
    b: &'a [u8],
    at: usize,
}

impl<'a> Bytes<'a> {
    fn byte(&mut self) -> Result<u8, String> {
        let v = *self.b.get(self.at).ok_or("the module ends too early")?;
        self.at += 1;
        Ok(v)
    }

    fn leb(&mut self) -> Result<u32, String> {
        let (mut value, mut shift) = (0u64, 0);
        loop {
            let byte = self.byte()?;
            value |= u64::from(byte & 0x7f) << shift;
            if byte & 0x80 == 0 {
                return u32::try_from(value)
                    .map_err(|_| "a number in the module is too large".into());
            }
            shift += 7;
            if shift > 35 {
                return Err("a number in the module is too long".into());
            }
        }
    }

    fn take(&mut self, n: usize) -> Result<&'a [u8], String> {
        let end = self.at.checked_add(n).filter(|&e| e <= self.b.len());
        let end = end.ok_or("the module ends too early")?;
        let s = &self.b[self.at..end];
        self.at = end;
        Ok(s)
    }

    fn name(&mut self) -> Result<String, String> {
        let n = self.leb()? as usize;
        Ok(String::from_utf8_lossy(self.take(n)?).into_owned())
    }

    fn limits(&mut self) -> Result<(), String> {
        let flags = self.byte()?;
        self.leb()?;
        if flags & 1 != 0 {
            self.leb()?;
        }
        Ok(())
    }
}

/// Reads the type, import, function and export sections of a module.
pub fn read_module(bytes: &[u8]) -> Result<ModuleInfo, String> {
    if bytes.len() < 8 || bytes[..4] != *b"\0asm" {
        return Err("the output is not a WebAssembly module".into());
    }
    if bytes[4..8] != [1, 0, 0, 0] {
        return Err("the module is not WebAssembly version 1".into());
    }
    let mut info = ModuleInfo::default();
    let mut r = Bytes { b: bytes, at: 8 };
    while r.at < bytes.len() {
        let id = r.byte()?;
        let size = r.leb()? as usize;
        let body = r.take(size)?;
        let mut s = Bytes { b: body, at: 0 };
        match id {
            1 => {
                for _ in 0..s.leb()? {
                    if s.byte()? != 0x60 {
                        return Err("the module has a type this check does not know".into());
                    }
                    let np = s.leb()? as usize;
                    let params = s.take(np)?.to_vec();
                    let nr = s.leb()? as usize;
                    let results = s.take(nr)?.to_vec();
                    info.types.push((params, results));
                }
            }
            2 => {
                for _ in 0..s.leb()? {
                    let module = s.name()?;
                    let name = s.name()?;
                    let kind = s.byte()?;
                    match kind {
                        KIND_FUNC => info.func_types.push(s.leb()?),
                        KIND_TABLE => {
                            s.byte()?;
                            s.limits()?;
                        }
                        KIND_MEMORY => s.limits()?,
                        KIND_GLOBAL => {
                            s.byte()?;
                            s.byte()?;
                        }
                        _ => {
                            s.byte()?;
                            s.leb()?;
                        }
                    }
                    info.imports.push(Import { module, name, kind });
                }
            }
            3 => {
                for _ in 0..s.leb()? {
                    info.func_types.push(s.leb()?);
                }
            }
            7 => {
                for _ in 0..s.leb()? {
                    let name = s.name()?;
                    let kind = s.byte()?;
                    let index = s.leb()?;
                    info.exports.push(Export { name, kind, index });
                }
            }
            _ => {}
        }
    }
    Ok(info)
}

/// The import names eidos_wasm.h declares, from its `EIDOS_IMPORT("name")` lines.
fn header_imports() -> impl Iterator<Item = &'static str> {
    HEADER.lines().filter_map(|line| {
        let rest = line.strip_prefix("EIDOS_IMPORT(\"")?;
        rest.split_once("\")").map(|(name, _)| name)
    })
}

/// Checks a built module against the contract. The messages are for the
/// person who wrote the mod.
pub fn check_module(bytes: &[u8], contract: &Contract) -> Result<ModuleInfo, String> {
    let info = read_module(bytes)?;
    for i in &info.imports {
        if i.kind == KIND_MEMORY {
            return Err("the module imports its memory; it has to export it".into());
        }
        if i.module != contract.import_module {
            return Err(format!(
                "the module imports {}.{}; a code mod may import only from \"{}\"",
                i.module, i.name, contract.import_module
            ));
        }
        if !header_imports().any(|n| n == i.name) {
            return Err(format!(
                "the module imports {}.{}, which eidos_wasm.h does not declare",
                i.module, i.name
            ));
        }
    }
    for (name, result) in &contract.export_funcs {
        let Some(e) = info
            .exports
            .iter()
            .find(|e| e.name == *name && e.kind == KIND_FUNC)
        else {
            return Err(format!("the module does not export the function {name}"));
        };
        let ty = info
            .func_types
            .get(e.index as usize)
            .and_then(|t| info.types.get(*t as usize));
        let wanted: &[u8] = match result.as_deref() {
            Some("i32") => &[0x7f],
            _ => &[],
        };
        if ty.is_none_or(|(params, results)| !params.is_empty() || results != wanted) {
            let shape = if result.is_some() {
                "returns an i32"
            } else {
                "returns nothing"
            };
            return Err(format!("{name} takes no arguments and {shape}"));
        }
    }
    if !contract.export_memory.is_empty()
        && !info
            .exports
            .iter()
            .any(|e| e.name == contract.export_memory && e.kind == KIND_MEMORY)
    {
        return Err(format!(
            "the module does not export its memory as \"{}\"",
            contract.export_memory
        ));
    }
    if contract.export_table && !info.exports.iter().any(|e| e.kind == KIND_TABLE) {
        return Err(
            "the module does not export its function table (link with --export-table)".into(),
        );
    }
    Ok(info)
}
