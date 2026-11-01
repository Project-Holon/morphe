//! Pokemon Black and White save files: who the player is, the team, the
//! badges and where they saved. Reads the 512 KiB image of the backup chip
//! that the game (and eidos) writes.
//!
//! The rules follow the decompiled save code (aletheia
//! `src/game/arm9/backup_system.c`, `savedata.c`, `save_control.c`):
//!
//! ```text
//! image:    two copies of the save data, at 0x00000 and 0x24000
//! copy:     0x23F9C bytes: the blocks, the CRC table, the footer
//! block:    payload, then u16 count and u16 CRC-16/CCITT of the payload;
//!           each block starts on a 0x100 boundary
//! CRC table at 0x23F00: one u16 per block, the CRC the block must have
//! footer:   the last 0x10 bytes: u32 save count (higher is newer),
//!           u32 data size (0x23F9C), u32 id, u16 pad, u16 CRC of the table
//! ```
//!
//! The game takes each block from the copy whose CRC holds, so this does
//! too, preferring the newer copy. The blocks read here, by id:
//!
//! ```text
//! 0x1A  party      0x18E00  0x534  u32 count at 4, then 6 x 220-byte Pokemon
//! 0x1B  system     0x19400  0x68   Config (4), MyStatus (0x20), PlayTime,
//!                                  u8 active (system_data.c)
//! 0x1C  location   0x19500  0x9C   u16 zone at 0x80: the location record
//!                                  Continue puts the player back on
//!                                  (player_location.c)
//! 0x34  misc       0x21200  0xEC   u32 badge bits at 4 (beacon.c MiscSave)
//! ```
//!
//! MyStatus (mystatus_types.h): u16 name[8], u32 id, u32 profile id, u8
//! nation, area, language, rom code, trainer view, sex. The active flag is
//! set while a new game runs and cleared by its first save
//! (SAVE_CONTROL_MainSave counts a save only when it is 0): a file with it
//! set holds no save the game will load.
//!
//! A Pokemon's first 136 bytes: u32 PID, u16 pad, u16 checksum, then four
//! 32-byte blocks shuffled by ((PID >> 13) & 31) % 24 and XORed with the
//! LCG 0x41C64E6D/0x6073 seeded with the checksum. The checksum is the sum
//! of the decrypted halfwords, so a wrong decode is caught. Species is the
//! first halfword of block A; the egg flag is bit 30 of the IV word, 0x10
//! into block B; the name is the first 11 units of block C. A party
//! Pokemon's bytes 0x88..0xDC (its battle stats) are XORed with the same
//! LCG seeded with the PID: level at 0x8C, HP at 0x8E, max HP at 0x90.

const COPY_SIZE: usize = 0x23F9C;
const SECOND_COPY: usize = 0x24000;
const CRC_TABLE: usize = 0x23F00;
const FOOTER: usize = COPY_SIZE - 0x10;
/// The image size the game writes.
pub const IMAGE_SIZE: usize = 0x80000;

struct Block {
    id: usize,
    offset: usize,
    size: usize,
}

const PARTY: Block = Block {
    id: 0x1A,
    offset: 0x18E00,
    size: 0x534,
};
const SYSTEM: Block = Block {
    id: 0x1B,
    offset: 0x19400,
    size: 0x68,
};
const LOCATION: Block = Block {
    id: 0x1C,
    offset: 0x19500,
    size: 0x9C,
};
const MISC: Block = Block {
    id: 0x34,
    offset: 0x21200,
    size: 0xEC,
};

const MON_SIZE: usize = 220;
const MON_BOX_SIZE: usize = 136;

/// The game a save belongs to (MyStatus rom code).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Game {
    Black,
    White,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Member {
    pub species: u16,
    pub egg: bool,
    /// The name the game shows: the nickname, or the species name in the
    /// save's language when it has none.
    pub name: String,
    pub level: u8,
    pub hp: u16,
    pub max_hp: u16,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Save {
    pub trainer: String,
    pub female: bool,
    /// The trainer ID shown on the trainer card.
    pub id: u16,
    pub game: Option<Game>,
    /// MyStatus language: 1 Japanese, 2 English, 3 French, 4 Italian,
    /// 5 German, 7 Spanish, 8 Korean.
    pub language: u8,
    /// Play time in seconds.
    pub seconds: u32,
    /// One bit per badge, in the order the game awards them.
    pub badges: u8,
    pub party: Vec<Member>,
    /// The zone the player saved in (see `place::place_names`).
    pub zone: u16,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Error {
    /// Not the size of a Black or White save image.
    Size(usize),
    /// Neither copy holds a block whose CRC is right.
    Broken(&'static str),
}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Error::Size(n) => write!(f, "not a Black or White save ({n} bytes)"),
            Error::Broken(what) => write!(f, "the save's {what} block is damaged in both copies"),
        }
    }
}

impl std::error::Error for Error {}

/// CRC-16/CCITT as MATH_CalcCRC16CCITT computes it (initial value 0xFFFF).
pub fn crc16_ccitt(data: &[u8]) -> u16 {
    let mut c: u16 = 0xFFFF;
    for &b in data {
        c ^= (b as u16) << 8;
        for _ in 0..8 {
            c = if c & 0x8000 != 0 {
                (c << 1) ^ 0x1021
            } else {
                c << 1
            };
        }
    }
    c
}

fn u16_at(b: &[u8], at: usize) -> u16 {
    u16::from_le_bytes([b[at], b[at + 1]])
}

fn u32_at(b: &[u8], at: usize) -> u32 {
    u32::from_le_bytes(b[at..at + 4].try_into().unwrap())
}

/// The copies in the order the game prefers them: the one with a sound
/// footer and the higher save count first.
fn copies(image: &[u8]) -> Vec<&[u8]> {
    let mut list: Vec<(u32, &[u8])> = [0, SECOND_COPY]
        .iter()
        .map(|&at| &image[at..at + COPY_SIZE])
        .filter(|c| {
            u32_at(c, FOOTER + 4) as usize == COPY_SIZE
                && u16_at(c, FOOTER + 0xE) == crc16_ccitt(&c[CRC_TABLE..FOOTER])
        })
        .map(|c| (u32_at(c, FOOTER), c))
        .collect();
    list.sort_by_key(|c| std::cmp::Reverse(c.0));
    list.into_iter().map(|(_, c)| c).collect()
}

fn block<'a>(copies: &[&'a [u8]], b: &Block, name: &'static str) -> Result<&'a [u8], Error> {
    copies
        .iter()
        .map(|c| (c, &c[b.offset..b.offset + b.size]))
        .find(|(c, payload)| {
            let want = u16_at(c, CRC_TABLE + b.id * 2);
            u16_at(c, b.offset + b.size + 2) == want && crc16_ccitt(payload) == want
        })
        .map(|(_, payload)| payload)
        .ok_or(Error::Broken(name))
}

/// The game's text: UTF-16 to the 0xFFFF end, its own gender marks mapped
/// to Unicode.
fn name(units: &[u8]) -> String {
    let chars: Vec<u16> = units
        .as_chunks::<2>()
        .0
        .iter()
        .map(|&c| u16::from_le_bytes(c))
        .take_while(|&c| c != 0xFFFF && c != 0)
        .map(|c| match c {
            0x246D => 0x2642,
            0x246E => 0x2640,
            c => c,
        })
        .collect();
    String::from_utf16_lossy(&chars)
}

const SHUFFLE: [[u8; 4]; 24] = [
    [0, 1, 2, 3],
    [0, 1, 3, 2],
    [0, 2, 1, 3],
    [0, 3, 1, 2],
    [0, 2, 3, 1],
    [0, 3, 2, 1],
    [1, 0, 2, 3],
    [1, 0, 3, 2],
    [2, 0, 1, 3],
    [3, 0, 1, 2],
    [2, 0, 3, 1],
    [3, 0, 2, 1],
    [1, 2, 0, 3],
    [1, 3, 0, 2],
    [2, 1, 0, 3],
    [3, 1, 0, 2],
    [2, 3, 0, 1],
    [3, 2, 0, 1],
    [1, 2, 3, 0],
    [1, 3, 2, 0],
    [2, 1, 3, 0],
    [3, 1, 2, 0],
    [2, 3, 1, 0],
    [3, 2, 1, 0],
];

/// One Pokemon of the party, or None for an empty or undecodable slot.
fn member(data: &[u8]) -> Option<Member> {
    let pid = u32_at(data, 0);
    let checksum = u16_at(data, 6);
    let mut words: Vec<u16> = data[8..MON_BOX_SIZE]
        .as_chunks::<2>()
        .0
        .iter()
        .map(|&c| u16::from_le_bytes(c))
        .collect();
    let mut seed = checksum as u32;
    for w in words.iter_mut() {
        seed = seed.wrapping_mul(0x41C6_4E6D).wrapping_add(0x6073);
        *w ^= (seed >> 16) as u16;
    }
    let sum = words.iter().fold(0u16, |a, &w| a.wrapping_add(w));
    if sum != checksum {
        return None;
    }
    // SHUFFLE[n][k]: the stored position of logical block k (A, B, C, D).
    let order = SHUFFLE[(((pid >> 13) & 31) % 24) as usize];
    let at = |k: usize, off: usize| order[k] as usize * 16 + off / 2;
    let species = words[at(0, 0)];
    let iv = words[at(1, 0x10)] as u32 | (words[at(1, 0x12)] as u32) << 16;
    let name_units: Vec<u8> = (0..11)
        .flat_map(|i| words[at(2, i * 2)].to_le_bytes())
        .collect();
    let mut stats: Vec<u16> = data[MON_BOX_SIZE..MON_SIZE]
        .as_chunks::<2>()
        .0
        .iter()
        .map(|&c| u16::from_le_bytes(c))
        .collect();
    let mut seed = pid;
    for w in stats.iter_mut() {
        seed = seed.wrapping_mul(0x41C6_4E6D).wrapping_add(0x6073);
        *w ^= (seed >> 16) as u16;
    }
    // stats[k] is the halfword at 0x88 + 2k
    (species != 0).then_some(Member {
        species,
        egg: iv >> 30 & 1 != 0,
        name: name(&name_units),
        level: stats[2] as u8,
        hp: stats[3],
        max_hp: stats[4],
    })
}

/// Reads a save image. `Ok(None)` when the file holds no save the game
/// would load: never written, or a new game left before its first save.
pub fn read(image: &[u8]) -> Result<Option<Save>, Error> {
    if image.len() < SECOND_COPY + COPY_SIZE {
        return Err(Error::Size(image.len()));
    }
    let copies = copies(image);
    if copies.is_empty() {
        return Ok(None);
    }
    let system = block(&copies, &SYSTEM, "player")?;
    if system[0x2C] != 0 {
        return Ok(None);
    }
    let status = &system[4..0x24];
    let party_block = block(&copies, &PARTY, "party")?;
    let count = (u32_at(party_block, 4) as usize).min(6);
    let party = (0..count)
        .filter_map(|i| member(&party_block[8 + i * MON_SIZE..8 + (i + 1) * MON_SIZE]))
        .collect();
    let location = block(&copies, &LOCATION, "location")?;
    let misc = block(&copies, &MISC, "progress")?;
    let hours = u16_at(system, 0x24) as u32;
    Ok(Some(Save {
        trainer: name(&status[0..16]),
        female: status[0x1D] != 0,
        id: u16_at(status, 0x10),
        game: match status[0x1B] {
            20 => Some(Game::White),
            21 => Some(Game::Black),
            _ => None,
        },
        language: status[0x1A],
        seconds: hours * 3600 + system[0x26] as u32 * 60 + system[0x27] as u32,
        badges: u32_at(misc, 4) as u8,
        party,
        zone: u16_at(location, 0x80),
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// An image with one copy: the given blocks with their CRCs, the CRC
    /// table and the footer.
    fn image(blocks: &[(&Block, Vec<u8>)], count: u32) -> Vec<u8> {
        let mut img = vec![0u8; IMAGE_SIZE];
        {
            let c = &mut img[..COPY_SIZE];
            for (b, payload) in blocks {
                c[b.offset..b.offset + b.size].copy_from_slice(payload);
                let crc = crc16_ccitt(payload);
                c[b.offset + b.size + 2..b.offset + b.size + 4].copy_from_slice(&crc.to_le_bytes());
                c[CRC_TABLE + b.id * 2..CRC_TABLE + b.id * 2 + 2]
                    .copy_from_slice(&crc.to_le_bytes());
            }
            c[FOOTER..FOOTER + 4].copy_from_slice(&count.to_le_bytes());
            c[FOOTER + 4..FOOTER + 8].copy_from_slice(&(COPY_SIZE as u32).to_le_bytes());
            let crc = crc16_ccitt(&c[CRC_TABLE..FOOTER]);
            c[FOOTER + 0xE..FOOTER + 0x10].copy_from_slice(&crc.to_le_bytes());
        }
        img
    }

    fn encrypt_mon(species: u16, pid: u32, egg: bool) -> Vec<u8> {
        let mut words = [0u16; 64];
        let order = SHUFFLE[(((pid >> 13) & 31) % 24) as usize];
        words[order[0] as usize * 16] = species;
        if egg {
            words[order[1] as usize * 16 + 9] = 0x4000;
        }
        for (i, c) in "Pig".encode_utf16().chain([0xFFFF]).enumerate() {
            words[order[2] as usize * 16 + i] = c;
        }
        let checksum = words.iter().fold(0u16, |a, &w| a.wrapping_add(w));
        let mut seed = checksum as u32;
        for w in words.iter_mut() {
            seed = seed.wrapping_mul(0x41C6_4E6D).wrapping_add(0x6073);
            *w ^= (seed >> 16) as u16;
        }
        let mut out = vec![0u8; MON_SIZE];
        out[0..4].copy_from_slice(&pid.to_le_bytes());
        out[6..8].copy_from_slice(&checksum.to_le_bytes());
        for (i, w) in words.iter().enumerate() {
            out[8 + i * 2..10 + i * 2].copy_from_slice(&w.to_le_bytes());
        }
        let mut stats = [0u16; 42];
        stats[2] = 12;
        stats[3] = 30;
        stats[4] = 35;
        let mut seed = pid;
        for (i, w) in stats.iter().enumerate() {
            seed = seed.wrapping_mul(0x41C6_4E6D).wrapping_add(0x6073);
            let v = w ^ (seed >> 16) as u16;
            out[MON_BOX_SIZE + i * 2..MON_BOX_SIZE + i * 2 + 2].copy_from_slice(&v.to_le_bytes());
        }
        out
    }

    fn sample(active: u8) -> Vec<u8> {
        let mut system = vec![0u8; SYSTEM.size];
        for (i, c) in "Sally".encode_utf16().chain([0xFFFF]).enumerate() {
            system[4 + i * 2..6 + i * 2].copy_from_slice(&c.to_le_bytes());
        }
        system[4 + 0x10..4 + 0x12].copy_from_slice(&43009u16.to_le_bytes());
        system[4 + 0x1A] = 2;
        system[4 + 0x1B] = 21;
        system[4 + 0x1D] = 1;
        system[0x24..0x26].copy_from_slice(&1u16.to_le_bytes());
        system[0x26] = 30;
        system[0x27] = 5;
        system[0x2C] = active;
        let mut party = vec![0u8; PARTY.size];
        party[4] = 2;
        party[8..8 + MON_SIZE].copy_from_slice(&encrypt_mon(498, 0x1234_5678, false));
        party[8 + MON_SIZE..8 + 2 * MON_SIZE].copy_from_slice(&encrypt_mon(504, 0x9ABC_DEF0, true));
        let mut location = vec![0u8; LOCATION.size];
        location[0x80..0x82].copy_from_slice(&397u16.to_le_bytes());
        let mut misc = vec![0u8; MISC.size];
        misc[4] = 0b0000_0111;
        image(
            &[
                (&SYSTEM, system),
                (&PARTY, party),
                (&LOCATION, location),
                (&MISC, misc),
            ],
            3,
        )
    }

    #[test]
    fn reads_the_player_the_team_and_the_place() {
        let s = read(&sample(0)).unwrap().unwrap();
        assert_eq!(s.trainer, "Sally");
        assert!(s.female);
        assert_eq!(s.id, 43009);
        assert_eq!(s.game, Some(Game::Black));
        assert_eq!(s.seconds, 3600 + 30 * 60 + 5);
        assert_eq!(s.badges.count_ones(), 3);
        let seen: Vec<_> = s.party.iter().map(|m| (m.species, m.egg)).collect();
        assert_eq!(seen, vec![(498, false), (504, true)]);
        let m = &s.party[0];
        assert_eq!(
            (m.name.as_str(), m.level, m.hp, m.max_hp),
            ("Pig", 12, 30, 35)
        );
        assert_eq!(s.zone, 397);
    }

    #[test]
    fn a_new_game_not_yet_saved_is_no_save() {
        assert_eq!(read(&sample(1)).unwrap(), None);
        assert_eq!(read(&vec![0u8; IMAGE_SIZE]).unwrap(), None);
    }

    #[test]
    fn a_damaged_copy_gives_way_to_the_other() {
        let mut img = sample(0);
        let good = img[..COPY_SIZE].to_vec();
        img[SECOND_COPY..SECOND_COPY + COPY_SIZE].copy_from_slice(&good);
        // newer first copy, its player block broken: the second copy's is read
        img[FOOTER..FOOTER + 4].copy_from_slice(&9u32.to_le_bytes());
        img[SYSTEM.offset + 4] ^= 0xFF;
        assert_eq!(read(&img).unwrap().unwrap().trainer, "Sally");
        img[SECOND_COPY + SYSTEM.offset + 4] ^= 0xFF;
        assert_eq!(read(&img), Err(Error::Broken("player")));
    }
}
