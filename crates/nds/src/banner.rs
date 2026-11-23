//! The cartridge banner: the 32x32 icon and the titles the DS menu shows.
//!
//! Version 1 and later share the start: CRC words (0x00-0x1F), the icon's
//! 4bpp tiles (0x20, 512 bytes, 4x4 tiles of 8x8), its 16-colour BGR555
//! palette (0x220, colour 0 transparent), then one 0x100-byte UTF-16 title
//! per language (0x240 Japanese, English, French, German, Italian, Spanish;
//! Chinese and Korean in later versions).

use crate::image::Rgba;
use crate::nclr::bgr555_to_rgba;

pub const LANGUAGES: [&str; 8] = [
    "Japanese", "English", "French", "German", "Italian", "Spanish", "Chinese", "Korean",
];

pub struct Banner {
    pub version: u16,
    /// The titles by language, as in [`LANGUAGES`]; lines are split by '\n'.
    pub titles: Vec<String>,
    pub icon: Rgba,
}

impl Banner {
    pub fn parse(bytes: &[u8]) -> Option<Banner> {
        if bytes.len() < 0x840 {
            return None;
        }
        let version = u16::from_le_bytes([bytes[0], bytes[1]]);
        let count = match version {
            0x0002 => 7,
            0x0003 | 0x0103 => 8,
            _ => 6,
        };
        let titles = (0..count)
            .filter_map(|i| bytes.get(0x240 + i * 0x100..0x340 + i * 0x100))
            .map(|raw| {
                let units: Vec<u16> = raw
                    .chunks(2)
                    .map(|c| u16::from_le_bytes([c[0], c[1]]))
                    .take_while(|&u| u != 0)
                    .collect();
                String::from_utf16_lossy(&units)
            })
            .collect();
        let palette: Vec<[u8; 4]> = (0..16)
            .map(|i| {
                let c = u16::from_le_bytes([bytes[0x220 + 2 * i], bytes[0x221 + 2 * i]]);
                if i == 0 {
                    [0, 0, 0, 0]
                } else {
                    bgr555_to_rgba(c)
                }
            })
            .collect();
        let mut icon = Rgba::new(32, 32);
        for ty in 0..4 {
            for tx in 0..4 {
                let tile = &bytes[0x20 + (ty * 4 + tx) * 32..][..32];
                for y in 0..8 {
                    for x in 0..8 {
                        let b = tile[y * 4 + x / 2];
                        let index = if x % 2 == 0 { b & 0x0F } else { b >> 4 };
                        let at = ((ty * 8 + y) * 32 + tx * 8 + x) * 4;
                        icon.pixels[at..at + 4].copy_from_slice(&palette[index as usize]);
                    }
                }
            }
        }
        Some(Banner {
            version,
            titles,
            icon,
        })
    }

    /// The English title.
    pub fn title(&self) -> &str {
        self.titles
            .get(1)
            .or(self.titles.first())
            .map_or("", String::as_str)
    }

    /// The title without its publisher line: by the DS convention a
    /// three-line title ends with the publisher. Lines are joined by spaces.
    pub fn name(&self) -> String {
        let lines: Vec<&str> = self
            .title()
            .lines()
            .map(str::trim)
            .filter(|l| !l.is_empty())
            .collect();
        let keep = if lines.len() >= 3 {
            &lines[..lines.len() - 1]
        } else {
            &lines[..]
        };
        keep.join(" ")
    }

    /// Whether the icon pictures (tiles and palette) of two banners differ.
    pub fn icon_differs(a: &[u8], b: &[u8]) -> bool {
        a.get(0x20..0x240) != b.get(0x20..0x240)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn titles_icon_and_name() {
        let mut b = vec![0u8; 0x840];
        b[0] = 1;
        // colour 1 = pure red; the first pixel of the first tile uses it
        b[0x222..0x224].copy_from_slice(&0x001Fu16.to_le_bytes());
        b[0x20] = 0x01;
        for (i, t) in [
            "Japanese title",
            "Pokemon Blaze Black\nFull Version\nNintendo & Drayano",
        ]
        .iter()
        .enumerate()
        {
            for (k, u) in t.encode_utf16().enumerate() {
                b[0x240 + i * 0x100 + 2 * k..][..2].copy_from_slice(&u.to_le_bytes());
            }
        }
        let banner = Banner::parse(&b).unwrap();
        assert_eq!(banner.titles.len(), 6);
        assert_eq!(banner.name(), "Pokemon Blaze Black Full Version");
        assert_eq!(banner.icon.get(0, 0), [255, 0, 0, 255]);
        assert_eq!(banner.icon.get(1, 0), [0, 0, 0, 0]);
        assert!(Banner::parse(&b[..0x100]).is_none());
    }
}
