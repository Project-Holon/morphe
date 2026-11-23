use super::*;

fn int(mut v: u64) -> Vec<u8> {
    let mut out = vec![(v & 0x7F) as u8];
    v >>= 7;
    while v != 0 {
        out.insert(0, 0x80 | (v & 0x7F) as u8);
        v >>= 7;
    }
    out
}

/// The default table's index of COPY `size` (0: the size follows) in `mode`.
fn copy(mode: u8, size: u8) -> u8 {
    19 + mode * 16 + if size == 0 { 0 } else { size - 3 }
}

struct Win<'a> {
    segment: Option<(u8, u64, u64)>,
    target: &'a [u8],
    data: Vec<u8>,
    inst: Vec<u8>,
    addr: Vec<u8>,
    adler: bool,
}

fn window(w: &Win) -> Vec<u8> {
    let mut ind = if w.adler { VCD_ADLER32 } else { 0 };
    let mut out = vec![];
    if let Some((kind, len, pos)) = w.segment {
        ind |= kind;
        out.extend(int(len));
        out.extend(int(pos));
    }
    let mut delta = int(w.target.len() as u64);
    delta.push(0);
    delta.extend(int(w.data.len() as u64));
    delta.extend(int(w.inst.len() as u64));
    delta.extend(int(w.addr.len() as u64));
    if w.adler {
        delta.extend(adler32(w.target).to_be_bytes());
    }
    delta.extend(&w.data);
    delta.extend(&w.inst);
    delta.extend(&w.addr);
    out.extend(int(delta.len() as u64));
    out.extend(delta);
    out.insert(0, ind);
    out
}

fn patch(app: Option<&[u8]>, windows: &[Win]) -> Vec<u8> {
    let mut p = MAGIC.to_vec();
    match app {
        Some(a) => {
            p.push(VCD_APPHEADER);
            p.extend(int(a.len() as u64));
            p.extend(a);
        }
        None => p.push(0),
    }
    for w in windows {
        p.extend(window(w));
    }
    p
}

#[test]
fn code_table_matches_the_rfc() {
    let t = default_code_table();
    assert_eq!(t.len(), 256);
    let first = |i: usize| (t[i][0].kind, t[i][0].size, t[i][0].mode);
    let second = |i: usize| (t[i][1].kind, t[i][1].size, t[i][1].mode);
    assert!(first(0) == (Kind::Run, 0, 0));
    assert!(first(1) == (Kind::Add, 0, 0) && first(18) == (Kind::Add, 17, 0));
    assert!(first(19) == (Kind::Copy, 0, 0) && first(34) == (Kind::Copy, 18, 0));
    assert!(first(147) == (Kind::Copy, 0, 8) && first(162) == (Kind::Copy, 18, 8));
    assert!(first(163) == (Kind::Add, 1, 0) && second(163) == (Kind::Copy, 4, 0));
    assert!(first(174) == (Kind::Add, 4, 0) && second(174) == (Kind::Copy, 6, 0));
    assert!(second(175) == (Kind::Copy, 4, 1) && second(234) == (Kind::Copy, 6, 5));
    assert!(first(235) == (Kind::Add, 1, 0) && second(235) == (Kind::Copy, 4, 6));
    assert!(first(246) == (Kind::Add, 4, 0) && second(246) == (Kind::Copy, 4, 8));
    assert!(first(247) == (Kind::Copy, 4, 0) && second(247) == (Kind::Add, 1, 0));
    assert!(first(255) == (Kind::Copy, 4, 8));
    assert_eq!(copy(1, 4), 36);
}

#[test]
fn add_run_and_overlapping_copy() {
    // "abc" + 5 x "x" + a copy of 6 from 0 (reads its own output: "abcxxx")
    // + 4 from HERE - 2 (overlaps: "xxxx")
    let target = b"abcxxxxxabcxxxxxxx";
    let p = patch(
        None,
        &[Win {
            segment: None,
            target,
            data: b"abcx".to_vec(),
            inst: vec![4, 0, 5, copy(0, 6), copy(1, 4)],
            addr: [int(0), int(2)].concat(),
            adler: true,
        }],
    );
    assert_eq!(apply(b"", &p).unwrap(), target);
}

#[test]
fn source_segment_and_address_caches() {
    let source = b"0123456789ABCDEF";
    // segment = "456789AB" (pos 4, len 8); addresses count from its start
    let target = b"6789789A789Az4567";
    let p = patch(
        Some(b"src//dst/"),
        &[Win {
            segment: Some((VCD_SOURCE, 8, 4)),
            target,
            data: b"z".to_vec(),
            // SELF 2; near[0] + 1 = 3; same[3] = 3; ADD 1 + COPY 4 SELF 0
            inst: vec![copy(0, 4), copy(2, 4), copy(6, 4), 163],
            addr: [int(2), int(1), vec![3], int(0)].concat(),
            adler: true,
        }],
    );
    assert_eq!(apply(source, &p).unwrap(), target);
    let i = info(&p).unwrap();
    assert_eq!(i.app_header.as_deref(), Some(&b"src//dst/"[..]));
    assert_eq!(
        (i.windows, i.target_size, i.source_needed, i.checksummed),
        (1, 17, 12, true)
    );
}

#[test]
fn several_windows_and_a_target_segment() {
    let w1 = Win {
        segment: None,
        target: b"hello",
        data: b"hello".to_vec(),
        inst: vec![6],
        addr: vec![],
        adler: false,
    };
    // window 2 copies from the output made so far (VCD_TARGET)
    let w2 = Win {
        segment: Some((VCD_TARGET, 5, 0)),
        target: b"hellohello",
        data: vec![],
        inst: vec![copy(0, 5), copy(0, 5)],
        addr: [int(0), int(0)].concat(),
        adler: true,
    };
    let p = patch(None, &[w1, w2]);
    assert_eq!(apply(b"", &p).unwrap(), b"hellohellohello");
    assert!(!info(&p).unwrap().checksummed);
}

#[test]
fn a_wrong_source_fails_its_checksum() {
    let p = patch(
        None,
        &[Win {
            segment: Some((VCD_SOURCE, 4, 0)),
            target: b"abcd",
            data: vec![],
            inst: vec![copy(0, 4)],
            addr: int(0),
            adler: true,
        }],
    );
    assert_eq!(apply(b"abcd", &p).unwrap(), b"abcd");
    assert_eq!(
        apply(b"abce", &p),
        Err(Error::Checksum {
            window: 0,
            offset: 0
        })
    );
    assert_eq!(apply(b"abc", &p), Err(Error::Segment { window: 0 }));
}

#[test]
fn refusals() {
    assert_eq!(apply(b"", b"PATCH"), Err(Error::NotVcdiff));
    let mut p = MAGIC.to_vec();
    p.extend([VCD_DECOMPRESS, 1]);
    assert_eq!(info(&p).unwrap().secondary, Some(1));
    // a window whose sections are compressed is refused
    let mut w = window(&Win {
        segment: None,
        target: b"a",
        data: b"a".to_vec(),
        inst: vec![2],
        addr: vec![],
        adler: false,
    });
    let len = w.len();
    w[len - 6] = 1; // Delta_Indicator: the data section is compressed
    p.extend(w);
    assert_eq!(apply(b"", &p), Err(Error::Secondary(1)));
    // cut short
    let full = patch(
        None,
        &[Win {
            segment: None,
            target: b"abc",
            data: b"abc".to_vec(),
            inst: vec![4],
            addr: vec![],
            adler: true,
        }],
    );
    assert_eq!(apply(b"", &full[..full.len() - 1]), Err(Error::Truncated));
    // a COPY that reads ahead
    let ahead = patch(
        None,
        &[Win {
            segment: None,
            target: b"abcd",
            data: vec![],
            inst: vec![copy(0, 4)],
            addr: int(0),
            adler: false,
        }],
    );
    assert!(matches!(apply(b"", &ahead), Err(Error::Bad { .. })));
}

#[test]
fn adler_matches_zlib() {
    // zlib.adler32(b"Wikipedia") == 0x11E60398
    assert_eq!(adler32(b"Wikipedia"), 0x11E6_0398);
    assert_eq!(adler32(b""), 1);
}
