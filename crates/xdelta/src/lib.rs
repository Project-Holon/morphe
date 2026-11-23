//! Applies xdelta3 patches: VCDIFF (RFC 3284) with xdelta3's extensions,
//! the per-window Adler-32 and the application header.
//!
//! A patch is a list of windows. Each window makes the next piece of the
//! output from three sections: the bytes ADD and RUN take (data), the
//! instructions (code table indices and sizes) and the COPY addresses. A
//! COPY reads the window's source segment (a part of the source file, or of
//! the output made so far) or the window's own output so far.
//!
//! Secondary compression (DJW, FGK, LZMA) is not read: a patch that uses it
//! is refused with [`Error::Secondary`]. The ROM hack patches made with
//! the xdelta UI tools do not use it.

use std::fmt;

/// The four VCDIFF magic bytes: "VCD" with the top bits set, version 0.
pub const MAGIC: [u8; 4] = [0xD6, 0xC3, 0xC4, 0x00];

// Hdr_Indicator
const VCD_DECOMPRESS: u8 = 0x01;
const VCD_CODETABLE: u8 = 0x02;
const VCD_APPHEADER: u8 = 0x04;
// Win_Indicator
const VCD_SOURCE: u8 = 0x01;
const VCD_TARGET: u8 = 0x02;
const VCD_ADLER32: u8 = 0x04;

const NEAR: usize = 4;
const SAME: usize = 3;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Error {
    /// The file does not start with the VCDIFF magic.
    NotVcdiff,
    /// The patch ends inside a header, a window or a section.
    Truncated,
    /// The patch uses a secondary compressor (1 DJW, 2 LZMA, 16 FGK).
    Secondary(u8),
    /// The patch brings its own code table.
    CodeTable,
    /// A window's source segment lies outside the source (or the output).
    Segment { window: usize },
    /// An instruction is wrong: a bad address, a size past the window, or
    /// sections that are not used up exactly.
    Bad { window: usize, what: &'static str },
    /// A window's output does not match its Adler-32: the source is not
    /// the file the patch was made from.
    Checksum { window: usize, offset: u64 },
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::NotVcdiff => write!(f, "not an xdelta patch (no VCDIFF header)"),
            Error::Truncated => write!(f, "the patch is cut short"),
            Error::Secondary(id) => {
                let name = match id {
                    1 => "DJW",
                    2 => "LZMA",
                    16 => "FGK",
                    _ => "an unknown",
                };
                write!(f, "the patch uses {name} secondary compression, which is not supported")
            }
            Error::CodeTable => write!(f, "the patch uses its own code table, which is not supported"),
            Error::Segment { window } => write!(f, "window {window}: its source segment is outside the source file"),
            Error::Bad { window, what } => write!(f, "window {window}: {what}"),
            Error::Checksum { window, offset } => write!(
                f,
                "window {window} (output offset {offset:#x}) does not match its checksum: the source file is not the one the patch was made for"
            ),
        }
    }
}

impl std::error::Error for Error {}

/// What the header and the windows say, without applying the patch.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Info {
    /// The secondary compressor id, if the patch uses one.
    pub secondary: Option<u8>,
    /// xdelta3's application data: usually "source//target/".
    pub app_header: Option<Vec<u8>>,
    pub windows: usize,
    /// The size of the output.
    pub target_size: u64,
    /// The end of the source the windows read: a source shorter than this
    /// cannot be right.
    pub source_needed: u64,
    /// Every window carries an Adler-32.
    pub checksummed: bool,
}

/// Reads the header and walks the windows.
pub fn info(patch: &[u8]) -> Result<Info, Error> {
    let mut r = Reader::new(patch);
    let header = read_header(&mut r)?;
    let mut info = Info {
        secondary: header.secondary,
        app_header: header.app_header,
        windows: 0,
        target_size: 0,
        source_needed: 0,
        checksummed: true,
    };
    while !r.done() {
        let w = read_window(&mut r, info.windows)?;
        if w.indicator & VCD_SOURCE != 0 {
            info.source_needed = info.source_needed.max(w.seg_pos + w.seg_len);
        }
        info.checksummed &= w.adler.is_some();
        info.target_size += w.target_len;
        info.windows += 1;
    }
    Ok(info)
}

/// Applies a patch to `source` and returns the output. A window that does
/// not match its Adler-32 is an error.
pub fn apply(source: &[u8], patch: &[u8]) -> Result<Vec<u8>, Error> {
    let (out, bad) = apply_checked(source, patch)?;
    match bad.first() {
        Some(m) => Err(Error::Checksum {
            window: m.window,
            offset: m.offset,
        }),
        None => Ok(out),
    }
}

/// A window whose output did not match its Adler-32.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Mismatch {
    pub window: usize,
    /// Where the window's output starts, and its length.
    pub offset: u64,
    pub len: u64,
}

/// Applies a patch and lists the windows that do not match their
/// checksums instead of stopping at the first. The caller decides whether
/// those parts matter: a ROM dumped with other filler after its data gives
/// mismatches only there.
pub fn apply_checked(source: &[u8], patch: &[u8]) -> Result<(Vec<u8>, Vec<Mismatch>), Error> {
    let mut bad = Vec::new();
    let mut r = Reader::new(patch);
    let header = read_header(&mut r)?;
    let table = default_code_table();
    let mut out: Vec<u8> = Vec::new();
    let mut index = 0usize;
    while !r.done() {
        let w = read_window(&mut r, index)?;
        if w.delta_indicator != 0 {
            // a section is secondary-compressed
            return Err(Error::Secondary(header.secondary.unwrap_or(0)));
        }
        let start = out.len();
        let segment: Vec<u8> = if w.indicator & (VCD_SOURCE | VCD_TARGET) != 0 {
            let from: &[u8] = if w.indicator & VCD_SOURCE != 0 {
                source
            } else {
                &out
            };
            let end = w
                .seg_pos
                .checked_add(w.seg_len)
                .ok_or(Error::Segment { window: index })?;
            if end > from.len() as u64 {
                return Err(Error::Segment { window: index });
            }
            from[w.seg_pos as usize..end as usize].to_vec()
        } else {
            Vec::new()
        };
        decode_window(&w, &segment, &table, &mut out, index)?;
        if let Some(sum) = w.adler {
            if adler32(&out[start..]) != sum {
                bad.push(Mismatch {
                    window: index,
                    offset: start as u64,
                    len: (out.len() - start) as u64,
                });
            }
        }
        index += 1;
    }
    Ok((out, bad))
}

/// Adler-32 (RFC 1950), as xdelta3 writes it per window.
pub fn adler32(bytes: &[u8]) -> u32 {
    const MOD: u32 = 65521;
    let (mut a, mut b) = (1u32, 0u32);
    for chunk in bytes.chunks(5552) {
        for &x in chunk {
            a += x as u32;
            b += a;
        }
        a %= MOD;
        b %= MOD;
    }
    (b << 16) | a
}

// ---------------------------------------------------------------------

struct Header {
    secondary: Option<u8>,
    app_header: Option<Vec<u8>>,
}

fn read_header(r: &mut Reader) -> Result<Header, Error> {
    if r.bytes(4).map_err(|_| Error::NotVcdiff)? != MAGIC {
        return Err(Error::NotVcdiff);
    }
    let ind = r.byte()?;
    let secondary = if ind & VCD_DECOMPRESS != 0 {
        Some(r.byte()?)
    } else {
        None
    };
    if ind & VCD_CODETABLE != 0 {
        return Err(Error::CodeTable);
    }
    let app_header = if ind & VCD_APPHEADER != 0 {
        let n = r.size()?;
        Some(r.bytes(n)?.to_vec())
    } else {
        None
    };
    Ok(Header {
        secondary,
        app_header,
    })
}

struct Window<'a> {
    indicator: u8,
    seg_len: u64,
    seg_pos: u64,
    target_len: u64,
    delta_indicator: u8,
    adler: Option<u32>,
    data: &'a [u8],
    inst: &'a [u8],
    addr: &'a [u8],
}

fn read_window<'a>(r: &mut Reader<'a>, window: usize) -> Result<Window<'a>, Error> {
    let indicator = r.byte()?;
    if indicator & VCD_SOURCE != 0 && indicator & VCD_TARGET != 0 {
        return Err(Error::Bad {
            window,
            what: "the window names both a source and a target segment",
        });
    }
    let (seg_len, seg_pos) = if indicator & (VCD_SOURCE | VCD_TARGET) != 0 {
        (r.int()?, r.int()?)
    } else {
        (0, 0)
    };
    let delta_len = r.size()?;
    let delta_start = r.pos;
    let target_len = r.int()?;
    let delta_indicator = r.byte()?;
    let data_len = r.size()?;
    let inst_len = r.size()?;
    let addr_len = r.size()?;
    let adler = if indicator & VCD_ADLER32 != 0 {
        let b = r.bytes(4)?;
        Some(u32::from_be_bytes([b[0], b[1], b[2], b[3]]))
    } else {
        None
    };
    let data = r.bytes(data_len)?;
    let inst = r.bytes(inst_len)?;
    let addr = r.bytes(addr_len)?;
    if r.pos - delta_start != delta_len {
        return Err(Error::Bad {
            window,
            what: "the delta length does not match its sections",
        });
    }
    Ok(Window {
        indicator,
        seg_len,
        seg_pos,
        target_len,
        delta_indicator,
        adler,
        data,
        inst,
        addr,
    })
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Kind {
    Noop,
    Add,
    Run,
    Copy,
}

#[derive(Clone, Copy)]
struct Inst {
    kind: Kind,
    size: u8,
    mode: u8,
}

const NOOP: Inst = Inst {
    kind: Kind::Noop,
    size: 0,
    mode: 0,
};

/// The default code table of RFC 3284, section 5.6.
fn default_code_table() -> Vec<[Inst; 2]> {
    let one = |kind, size, mode| [Inst { kind, size, mode }, NOOP];
    let mut t = vec![one(Kind::Run, 0, 0)];
    for size in 0..=17 {
        t.push(one(Kind::Add, size, 0));
    }
    for mode in 0..9 {
        t.push(one(Kind::Copy, 0, mode));
        for size in 4..=18 {
            t.push(one(Kind::Copy, size, mode));
        }
    }
    for mode in 0..6 {
        for add in 1..=4 {
            for copy in 4..=6 {
                t.push([
                    Inst {
                        kind: Kind::Add,
                        size: add,
                        mode: 0,
                    },
                    Inst {
                        kind: Kind::Copy,
                        size: copy,
                        mode,
                    },
                ]);
            }
        }
    }
    for mode in 6..9 {
        for add in 1..=4 {
            t.push([
                Inst {
                    kind: Kind::Add,
                    size: add,
                    mode: 0,
                },
                Inst {
                    kind: Kind::Copy,
                    size: 4,
                    mode,
                },
            ]);
        }
    }
    for mode in 0..9 {
        t.push([
            Inst {
                kind: Kind::Copy,
                size: 4,
                mode,
            },
            Inst {
                kind: Kind::Add,
                size: 1,
                mode: 0,
            },
        ]);
    }
    debug_assert_eq!(t.len(), 256);
    t
}

fn decode_window(
    w: &Window,
    segment: &[u8],
    table: &[[Inst; 2]],
    out: &mut Vec<u8>,
    window: usize,
) -> Result<(), Error> {
    let bad = |what| Error::Bad { window, what };
    let start = out.len();
    let target_len = usize::try_from(w.target_len).map_err(|_| bad("the window is too large"))?;
    let s = segment.len() as u64;
    let mut data = Reader::new(w.data);
    let mut inst = Reader::new(w.inst);
    let mut addr = Reader::new(w.addr);
    let mut near = [0u64; NEAR];
    let mut next_near = 0usize;
    let mut same = [0u64; SAME * 256];
    out.reserve(target_len);

    while !inst.done() {
        let entry = table[inst.byte()? as usize];
        for i in entry {
            if i.kind == Kind::Noop {
                continue;
            }
            let size = if i.size == 0 {
                inst.size()?
            } else {
                i.size as usize
            };
            let made = out.len() - start;
            if made + size > target_len {
                return Err(bad("an instruction writes past the window"));
            }
            match i.kind {
                Kind::Noop => {}
                Kind::Add => out.extend_from_slice(
                    data.bytes(size)
                        .map_err(|_| bad("the data section is too short"))?,
                ),
                Kind::Run => {
                    let b = data
                        .byte()
                        .map_err(|_| bad("the data section is too short"))?;
                    out.resize(out.len() + size, b);
                }
                Kind::Copy => {
                    let here = s + made as u64;
                    let a = match i.mode as usize {
                        0 => addr.int()?,
                        1 => here
                            .checked_sub(addr.int()?)
                            .ok_or_else(|| bad("a HERE address is negative"))?,
                        m if m < 2 + NEAR => near[m - 2]
                            .checked_add(addr.int()?)
                            .ok_or_else(|| bad("an address overflows"))?,
                        m if m < 2 + NEAR + SAME => {
                            same[(m - 2 - NEAR) * 256 + addr.byte()? as usize]
                        }
                        _ => return Err(bad("an unknown address mode")),
                    };
                    if a >= here {
                        return Err(bad("a COPY reads ahead of the output"));
                    }
                    near[next_near] = a;
                    next_near = (next_near + 1) % NEAR;
                    same[(a % (SAME as u64 * 256)) as usize] = a;
                    let a = a as usize;
                    let seg = segment.len();
                    for k in 0..size {
                        let p = a + k;
                        let b = if p < seg {
                            segment[p]
                        } else {
                            out[start + (p - seg)]
                        };
                        out.push(b);
                    }
                }
            }
        }
    }
    if out.len() - start != target_len {
        return Err(bad(
            "the instructions make a different size than the window states",
        ));
    }
    if !data.done() || !addr.done() {
        return Err(bad("the data or address section is not used up"));
    }
    Ok(())
}

struct Reader<'a> {
    buf: &'a [u8],
    pos: usize,
}

impl<'a> Reader<'a> {
    fn new(buf: &'a [u8]) -> Self {
        Reader { buf, pos: 0 }
    }

    fn done(&self) -> bool {
        self.pos >= self.buf.len()
    }

    fn byte(&mut self) -> Result<u8, Error> {
        let b = *self.buf.get(self.pos).ok_or(Error::Truncated)?;
        self.pos += 1;
        Ok(b)
    }

    fn bytes(&mut self, n: usize) -> Result<&'a [u8], Error> {
        let end = self.pos.checked_add(n).ok_or(Error::Truncated)?;
        let s = self.buf.get(self.pos..end).ok_or(Error::Truncated)?;
        self.pos = end;
        Ok(s)
    }

    /// A VCDIFF integer: base 128, big end first, top bit set on all but the last byte.
    fn int(&mut self) -> Result<u64, Error> {
        let mut v = 0u64;
        for _ in 0..10 {
            let b = self.byte()?;
            v = v.checked_mul(128).ok_or(Error::Truncated)? | (b & 0x7F) as u64;
            if b & 0x80 == 0 {
                return Ok(v);
            }
        }
        Err(Error::Truncated)
    }

    fn size(&mut self) -> Result<usize, Error> {
        usize::try_from(self.int()?).map_err(|_| Error::Truncated)
    }
}

#[cfg(test)]
mod tests;
