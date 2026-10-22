//! Checks for the code-mod SDK: the module check against the contract and
//! the header reader.

use std::path::Path;

use eidos_mod::modcode;
use eidos_mod::sdk::*;

// --- handcrafted modules ---

fn leb(mut v: u32) -> Vec<u8> {
    let mut out = Vec::new();
    loop {
        let b = (v & 0x7f) as u8;
        v >>= 7;
        if v == 0 {
            out.push(b);
            return out;
        }
        out.push(b | 0x80);
    }
}

fn name(s: &str) -> Vec<u8> {
    let mut out = leb(s.len() as u32);
    out.extend(s.as_bytes());
    out
}

fn section(id: u8, body: Vec<u8>) -> Vec<u8> {
    let mut out = vec![id];
    out.extend(leb(body.len() as u32));
    out.extend(body);
    out
}

struct Module {
    import_module: &'static str,
    start_result: bool,
    export_memory: bool,
    export_table: bool,
    export_stop: bool,
    import_memory: bool,
    import_name: &'static str,
}

impl Module {
    fn good() -> Self {
        Module {
            import_module: "eidos",
            start_result: true,
            export_memory: true,
            export_table: true,
            export_stop: true,
            import_memory: false,
            import_name: "log",
        }
    }

    fn bytes(&self) -> Vec<u8> {
        let mut m = b"\0asm\x01\0\0\0".to_vec();
        // Types: 0 = (i32, i32) -> (), 1 = () -> i32, 2 = () -> ().
        m.extend(section(
            1,
            [
                vec![3],
                vec![0x60, 2, 0x7f, 0x7f, 0],
                vec![0x60, 0, 1, 0x7f],
                vec![0x60, 0, 0],
            ]
            .concat(),
        ));
        let mut imports = vec![if self.import_memory { 2 } else { 1 }];
        imports.extend(name(self.import_module));
        imports.extend(name(self.import_name));
        imports.extend([0, 0]);
        if self.import_memory {
            imports.extend(name("env"));
            imports.extend(name("memory"));
            imports.extend([2, 0, 1]);
        }
        m.extend(section(2, imports));
        // Functions: start, stop (func indices 1 and 2).
        let start_type = if self.start_result { 1 } else { 2 };
        m.extend(section(3, vec![2, start_type, 2]));
        m.extend(section(4, vec![1, 0x70, 0, 1]));
        m.extend(section(5, vec![1, 0, 1]));
        let mut exports = Vec::new();
        let mut count = 0;
        let mut add = |n: &str, kind: u8, index: u8| {
            exports.extend(name(n));
            exports.extend([kind, index]);
            count += 1;
        };
        if self.export_memory {
            add("memory", 2, 0);
        }
        add("eidos_mod_start", 0, 1);
        if self.export_stop {
            add("eidos_mod_stop", 0, 2);
        }
        if self.export_table {
            add("__indirect_function_table", 1, 0);
        }
        m.extend(section(7, [vec![count], exports].concat()));
        m
    }
}

#[test]
fn module_check() {
    let c = contract();
    let info = check_module(&Module::good().bytes(), &c).unwrap();
    assert_eq!(info.imports.len(), 1);
    assert_eq!(info.exports.len(), 4);

    let fail = |m: Module| check_module(&m.bytes(), &c).unwrap_err();
    assert!(fail(Module {
        import_module: "env",
        ..Module::good()
    })
    .contains("\"eidos\""));
    assert!(fail(Module {
        import_name: "not_a_call",
        ..Module::good()
    })
    .contains("not_a_call"));
    assert!(fail(Module {
        start_result: false,
        ..Module::good()
    })
    .contains("eidos_mod_start"));
    assert!(fail(Module {
        export_memory: false,
        ..Module::good()
    })
    .contains("memory"));
    assert!(fail(Module {
        export_table: false,
        ..Module::good()
    })
    .contains("table"));
    assert!(fail(Module {
        export_stop: false,
        ..Module::good()
    })
    .contains("eidos_mod_stop"));
    assert!(fail(Module {
        import_memory: true,
        ..Module::good()
    })
    .contains("memory"));

    assert!(check_module(b"", &c).is_err());
    assert!(check_module(b"MZ\x90\0\x03\0\0\0", &c)
        .unwrap_err()
        .contains("WebAssembly"));
    let good = Module::good().bytes();
    assert!(check_module(&good[..good.len() - 3], &c).is_err());
}

// --- the header ---

#[test]
fn header_names() {
    let api = code_api(Path::new("no/such/mod"));
    let f = |n: &str| api.functions.iter().find(|f| f.name == n).unwrap();
    assert_eq!(
        f("eidos_log").signature,
        "void eidos_log(const char *text, uint32_t len)"
    );
    // The note after a declaration is its doc.
    assert_eq!(f("eidos_log").doc, "a line in the mod log");
    assert_eq!(
        f("eidos_override").signature,
        "int32_t eidos_override(uint32_t region, uint32_t addr, uint32_t handler_index)"
    );
    // Every core import is there, with its note.
    for name in [
        "log", "on", "script", "override", "original", "rd8", "rd16", "rd32", "wr8", "wr16",
        "wr32", "read", "write",
    ] {
        let full = format!("eidos_{name}");
        assert!(!f(&full).doc.is_empty(), "{full}");
    }
    assert_eq!(
        f("eidos_original").signature,
        "int64_t eidos_original(const uint32_t *args, uint32_t nargs)"
    );
    assert_eq!(
        f("eidos_read").signature,
        "int32_t eidos_read(uint32_t ds_addr, void *dst, uint32_t len)"
    );
    assert_eq!(c_value(&api, "EIDOS_CURE_BATTLE"), "1");
    assert_eq!(c_value(&api, "EIDOS_EVENT_MON_FORM"), "56");
    assert!(api.functions.iter().any(|f| f.name == "eidos_on"));
    assert!(api.functions.iter().any(|f| f.name == "eidos_species_base"));
    let c = |n: &str| api.constants.iter().find(|c| c.name == n).unwrap();
    assert_eq!(c("EIDOS_EVENT_BATTLE_START").value, "70");
    assert_eq!(c("EIDOS_EVENT_CONTINUE").value, "0");
    assert_eq!(c("EIDOS_EVENT_MON_REVIVE").value, "96");

    // Every event the rule language names has an id in the header.
    let ids: Vec<_> = api.constants.iter().map(|c| c.name.as_str()).collect();
    let missing: Vec<_> = modcode::rule_names()
        .events
        .iter()
        .map(|e| format!("EIDOS_EVENT_{}", e.name.replace('.', "_").to_uppercase()))
        .filter(|n| !ids.contains(&n.as_str()))
        .collect();
    assert!(missing.is_empty(), "{missing:?}");
}

fn c_value(api: &CodeApi, name: &str) -> String {
    api.constants
        .iter()
        .find(|c| c.name == name)
        .unwrap()
        .value
        .clone()
}
