//! A mod's options file: the player-facing settings the manifest's
//! `settings` key points at.
//!
//! The grammar follows eidos's `sdk/mod_menu.c` (2ac9d2c, with the version 7
//! lines of eidos docs/mod-services.md):
//!
//! ```text
//! option <key> bool   "<label>" on|off
//! option <key> choice "<label>" first|second|third
//! option <key> int    "<label>" <min> <max> <default>
//! option <key> text   "<label>" <longest> "<default>" [word]
//! option <key> secret "<label>" <longest>
//! heading "<text>"
//! help <key> "<text>"
//! tab "<title>"
//! button <key> "<label>"
//! show <key> "<label>"
//! ```
//!
//! - a line is cut at the first `#` outside quotes (a quote toggles, so a
//!   `#` inside a quoted label is text); a line that starts with another
//!   word (`mode`, `yaml`, anything else) is kept as it is;
//! - the key and the kind are words, the label is a quoted text (or a word),
//!   and the value is the rest of the line;
//! - option, button and show lines may end with two words, in either order:
//!   `?key=value` or `?key!=value` (the row shows only while the option or
//!   show row `key` has that value, or not; an empty value is "empty"), and
//!   `@lobby` or `@<mode>,<mode>` (a co-op lobby rule, or the modes that
//!   ask it). A `?` or `@` that is not one of the last words, or is inside
//!   quotes, is part of the value;
//! - a key is one word under 64 characters with no space, `=`, `#` or `"`;
//!   dots are allowed;
//! - a bool is `on`, `off`, `true`, `false`, `1` or `0`; any other text is
//!   reported and the line is ignored;
//! - a choice lists names of up to 31 characters, split on `|`; empty names
//!   are dropped and the first name is the default;
//! - an int has three whole numbers within a billion of zero, with
//!   `min <= default <= max`;
//! - a text has a longest length from 1 to 64 and a default (it may be left
//!   out) no longer than that, without `"`; a last word `word` makes it one
//!   word (the game's keyboard has no space key); a secret (a password) has only
//!   its longest length, is never written to disk and is never `@lobby`;
//! - everything after a `tab` line goes on a Settings tab of its own until
//!   the next `tab` line; a title holds 31 characters;
//! - a `button` row sends the event setting.button to the mod; a `show` row
//!   is text the mod sets, hidden while it is empty;
//! - a key declared twice (by any option, button or show line) is reported
//!   and the second line is ignored; a `help` line names an option above it;
//! - there is no fixed number of options or names; the port stops at 4096
//!   options and 1024 names of a choice, only to guard against a broken
//!   file; labels are cut at 63 characters.
//!
//! The port ignores a line it refuses. The player's choices are stored as
//! `key = number` lines, so a key has no `=`. A rule reads a bool, choice or
//! int with `setting.<key>`, and a dotted key is read as written
//! (`setting.nuzlocke.dupes`).
//!
//! [`parse_options`] and [`render_options`] round-trip a file: comments, blank
//! lines, the order and the text of an unchanged line stay as they are.

use std::collections::BTreeSet;
use std::fs;
use std::io;
use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::manifest::Folder;
use crate::modcode::{Diagnostic, Severity};

/// The most options one mod has (a guard against a broken file); the port
/// ignores the rest.
pub const OPTIONS_MAX: usize = 4096;
/// The longest key and label, in bytes (the port keeps 63).
pub const KEY_MAX: usize = 63;
pub const LABEL_MAX: usize = 63;
/// The most names of a choice (a guard), and the longest name, in bytes.
pub const CHOICES_MAX: usize = 1024;
pub const CHOICE_LEN_MAX: usize = 31;
/// The widest an int bound goes.
pub const INT_LIMIT: i32 = 1_000_000_000;
/// The longest a text or secret option may be.
pub const TEXT_MAX: u32 = 64;
/// The longest tab title.
pub const TAB_TITLE_MAX: usize = 31;
/// The bytes of a line the port reads.
const LINE_MAX: usize = 4095;
/// The file a new options list goes in.
pub const OPTIONS_FILE: &str = "options.txt";

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum OptionKind {
    #[default]
    Bool,
    Choice,
    Int,
    Text,
    Secret,
}

impl OptionKind {
    fn word(self) -> &'static str {
        match self {
            OptionKind::Bool => "bool",
            OptionKind::Choice => "choice",
            OptionKind::Int => "int",
            OptionKind::Text => "text",
            OptionKind::Secret => "secret",
        }
    }

    /// The kinds a rule (`setting.<key>`) and `setting_get` read.
    pub fn numeric(self) -> bool {
        matches!(
            self,
            OptionKind::Bool | OptionKind::Choice | OptionKind::Int
        )
    }
}

/// What a line of the file is.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Entry {
    #[default]
    Option,
    Heading,
    Help,
    Tab,
    Button,
    Show,
}

impl Entry {
    fn word(self) -> &'static str {
        match self {
            Entry::Option => "option",
            Entry::Heading => "heading",
            Entry::Help => "help",
            Entry::Tab => "tab",
            Entry::Button => "button",
            Entry::Show => "show",
        }
    }

    /// The lines that declare a key of their own.
    fn declares(self) -> bool {
        matches!(self, Entry::Option | Entry::Button | Entry::Show)
    }
}

/// One line of the file the editor knows: an option, or a heading, help,
/// tab, button or show line (`entry`).
///
/// - an option: `key`, `label`, `kind` and its value. `default` is 0 or 1
///   for a bool, 0 for a choice (the first name) and the number for an int;
///   for a text and a secret `max` is the longest length and `text` a
///   text's default;
/// - a heading or a tab: `label` is its text;
/// - a help line: `key` names the option, `label` is the text;
/// - a button or a show row: `key` and `label`.
///
/// `when` is the condition without its `?` (`ra.who!=`), `tags` the `@`
/// word without its `@` (`lobby`); both are empty when absent.
///
/// `line`, `lead`, `comment` and `raw` come from the file and go back to
/// [`render_options`] unchanged: they keep the comments and the text of a
/// line nobody edited.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct OptionDecl {
    pub entry: Entry,
    pub key: String,
    pub label: String,
    pub kind: OptionKind,
    pub default: i32,
    pub min: i32,
    pub max: i32,
    pub values: Vec<String>,
    pub text: String,
    /// A text option of one word: the game's keyboard has no space key.
    pub word: bool,
    pub when: String,
    pub tags: String,
    /// 1-based line in the file, 0 for a new option.
    pub line: u32,
    /// The comment lines right above the option.
    pub lead: Vec<String>,
    /// The comment after the value, with its `#`.
    pub comment: String,
    /// The line as the file has it.
    pub raw: String,
}

/// What a file holds: the options, and the problems of the lines the port
/// skips and of the options themselves.
#[derive(Debug, Clone, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Parsed {
    pub options: Vec<OptionDecl>,
    pub diagnostics: Vec<Diagnostic>,
}

// ---------------------------------------------------------------------
// Reading
// ---------------------------------------------------------------------

fn is_space(c: char) -> bool {
    matches!(c, ' ' | '\t' | '\r' | '\x0b' | '\x0c' | '\n')
}

/// The next token of `s` from `at`: a quoted text or a word. Returns the
/// text and the byte after it, or `None` at the end of the line.
fn next_token(s: &str, mut at: usize) -> Option<(&str, usize)> {
    let b = s.as_bytes();
    while at < b.len() && is_space(b[at] as char) {
        at += 1;
    }
    if at >= b.len() {
        return None;
    }
    if b[at] == b'"' {
        let start = at + 1;
        let end = s[start..].find('"').map_or(s.len(), |i| start + i);
        let after = if end < s.len() { end + 1 } else { end };
        return Some((&s[start..end], after));
    }
    let end = s[at..].find(is_space).map_or(s.len(), |i| at + i);
    Some((&s[at..end], end))
}

/// The line the port reads: cut at 4095 bytes (on a character boundary),
/// then at the first `#` outside quotes. Returns the body and the comment
/// (with its `#`).
fn split_comment(line: &str) -> (&str, &str) {
    let mut cut = line.len().min(LINE_MAX);
    while !line.is_char_boundary(cut) {
        cut -= 1;
    }
    let read = &line[..cut];
    let mut quoted = false;
    for (i, b) in read.bytes().enumerate() {
        match b {
            b'"' => quoted = !quoted,
            b'#' if !quoted => return (&read[..i], line[i..].trim_end()),
            _ => {}
        }
    }
    (read, "")
}

/// The last word of `value` when it starts with `mark` after a space (or at
/// the start, for `@`) outside quotes; `None` when the last such mark is not
/// in the last word. As the port's `cut_when` and `cut_mode_tags`.
fn cut_last(value: &str, mark: u8) -> Option<(&str, &str)> {
    let b = value.as_bytes();
    let mut quoted = false;
    let mut found = None;
    for (i, &c) in b.iter().enumerate() {
        if c == b'"' {
            quoted = !quoted;
        } else if c == mark
            && !quoted
            && ((i > 0 && is_space(b[i - 1] as char)) || (i == 0 && mark == b'@'))
        {
            found = Some(i);
        }
    }
    let at = found?;
    let word = &value[at + 1..];
    if word.contains(is_space) {
        return None;
    }
    if mark == b'?' && !word.contains('=') {
        return None;
    }
    Some((value[..at].trim_end(), word))
}

/// Cuts the condition and the `@` word off the end of a value, in either
/// order: `(value, when, tags)`.
fn cut_ends(value: &str) -> (&str, String, String) {
    let mut v = value;
    let mut when = String::new();
    let mut tags = String::new();
    if let Some((rest, w)) = cut_last(v, b'?') {
        v = rest;
        when = w.to_string();
    }
    if let Some((rest, t)) = cut_last(v, b'@') {
        v = rest;
        tags = t.to_string();
    }
    if when.is_empty() {
        if let Some((rest, w)) = cut_last(v, b'?') {
            v = rest;
            when = w.to_string();
        }
    }
    (v, when, tags)
}

/// A bool's value: `None` for text the port reports and ignores.
fn bool_value(text: &str) -> Option<bool> {
    match text {
        "on" | "true" | "1" => Some(true),
        "off" | "false" | "0" => Some(false),
        _ => None,
    }
}

/// Is `key` a word the port accepts: under 64 bytes, no space, `=`, `#` or
/// `"`.
fn key_valid(key: &str) -> bool {
    !key.is_empty()
        && key.len() <= KEY_MAX
        && !key.contains(|c: char| is_space(c) || matches!(c, '"' | '#' | '='))
}

/// A whole number the way `strtol` reads it in base 10: an optional sign and
/// digits. Returns it and the rest.
fn long_prefix(s: &str) -> Option<(i64, &str)> {
    let t = s.trim_start_matches(is_space);
    let digits = t.strip_prefix(['-', '+']).unwrap_or(t);
    let n = digits.bytes().take_while(u8::is_ascii_digit).count();
    if n == 0 {
        return None;
    }
    let sign_len = t.len() - digits.len();
    let v: i64 =
        t[..sign_len + n]
            .trim_start_matches('+')
            .parse()
            .unwrap_or(if t.starts_with('-') {
                i64::MIN
            } else {
                i64::MAX
            });
    Some((v, &t[sign_len + n..]))
}

type LineError = (usize, usize, String);

/// One line. `Ok(None)` is a line the editor keeps as it is (blank, a
/// comment, `mode`, `yaml`, another word), `Err` is a line the port refuses.
fn parse_line(line: &str) -> Result<Option<OptionDecl>, LineError> {
    let (body, comment) = split_comment(line);
    let Some((word, at)) = next_token(body, 0) else {
        return Ok(None);
    };
    let whole = (0, body.trim_end().len());
    let entry = match word {
        "option" => Entry::Option,
        "heading" => Entry::Heading,
        "help" => Entry::Help,
        "tab" => Entry::Tab,
        "button" => Entry::Button,
        "show" => Entry::Show,
        _ => return Ok(None),
    };
    let mut o = OptionDecl {
        entry,
        comment: comment.to_string(),
        raw: line.to_string(),
        ..OptionDecl::default()
    };
    let missing = |what: &str, form: &str| -> LineError {
        (
            whole.0,
            whole.1,
            format!(
                "the {} line has no {what}: {form}; the game skips it",
                entry.word()
            ),
        )
    };
    match entry {
        Entry::Heading | Entry::Tab => {
            let form = if entry == Entry::Tab {
                "tab \"<title>\""
            } else {
                "heading \"<text>\""
            };
            let (text, _) = next_token(body, at).ok_or_else(|| missing("text", form))?;
            o.label = text.to_string();
            return Ok(Some(o));
        }
        Entry::Help => {
            let form = "help <key> \"<text>\"";
            let (key, at) = next_token(body, at).ok_or_else(|| missing("key", form))?;
            let (text, _) = next_token(body, at).ok_or_else(|| missing("text", form))?;
            o.key = key.to_string();
            o.label = text.to_string();
            return Ok(Some(o));
        }
        Entry::Button | Entry::Show => {
            let form = format!("{} <key> \"<label>\"", entry.word());
            let (key, at) = next_token(body, at).ok_or_else(|| missing("key", &form))?;
            let (label, at) = next_token(body, at).ok_or_else(|| missing("label", &form))?;
            if !key_valid(key) {
                return Err(bad_key(whole, key));
            }
            // the space after the label stays, so a `?` or `@` right after
            // it is a last word
            let (rest, when, tags) = cut_ends(body[at..].trim_end());
            let rest = rest.trim();
            if !rest.is_empty() {
                return Err((
                    whole.0,
                    whole.1,
                    format!(
                        "the {} line '{key}' has '{rest}' after its label; the game skips it",
                        entry.word()
                    ),
                ));
            }
            o.key = key.to_string();
            o.label = label.to_string();
            o.when = when;
            o.tags = tags;
            return Ok(Some(o));
        }
        Entry::Option => {}
    }
    let mut at = at;
    let mut take = |what: &str| -> Result<(&str, usize, usize), LineError> {
        let start = at;
        match next_token(body, at) {
            Some((t, next)) => {
                at = next;
                Ok((t, start, next))
            }
            None => Err((
                whole.0,
                whole.1,
                format!(
                    "the option line has no {what}: option <key> <bool|choice|int|text|secret> \"<label>\" <value>; the game skips it"
                ),
            )),
        }
    };
    let (key, _, _) = take("key")?;
    let (kind_word, kind_from, kind_to) = take("kind")?;
    let (label, _, _) = take("label")?;
    if !key_valid(key) {
        return Err(bad_key(whole, key));
    }
    let (value, when, tags) = cut_ends(body[at..].trim());
    let value_at = body[at..].find(value).map_or(at, |i| at + i);
    o.key = key.to_string();
    o.label = label.to_string();
    o.when = when;
    o.tags = tags;
    let bad = |msg: String| (value_at, value_at + value.len().max(1), msg);
    match kind_word {
        "bool" => {
            o.kind = OptionKind::Bool;
            let Some(on) = bool_value(value) else {
                return Err(bad(format!(
                    "the bool option '{key}' has the value '{value}'; it takes on or off; the game ignores it"
                )));
            };
            o.default = i32::from(on);
        }
        "choice" => {
            o.kind = OptionKind::Choice;
            o.values = value
                .split('|')
                .map(str::trim)
                .filter(|n| !n.is_empty())
                .map(str::to_string)
                .collect();
            if o.values.is_empty() {
                return Err(bad(format!(
                    "the option '{key}' names no choice: list the names with '|'; the game skips it"
                )));
            }
        }
        "int" => {
            let need = format!(
                "the int option '{key}' needs '<min> <max> <default>' with min <= default <= max; the game skips it"
            );
            let Some((lo, rest)) = long_prefix(value) else {
                return Err(bad(need));
            };
            let Some((hi, rest)) = long_prefix(rest) else {
                return Err(bad(need));
            };
            let Some((def, rest)) = long_prefix(rest) else {
                return Err(bad(need));
            };
            let limit = i64::from(INT_LIMIT);
            if !rest.trim().is_empty()
                || lo > hi
                || def < lo
                || def > hi
                || lo < -limit
                || hi > limit
            {
                return Err(bad(need));
            }
            o.kind = OptionKind::Int;
            o.min = lo as i32;
            o.max = hi as i32;
            o.default = def as i32;
        }
        "text" | "secret" => {
            let secret = kind_word == "secret";
            let need = if secret {
                format!("the secret option '{key}' needs '<longest>' from 1 to {TEXT_MAX}; the game skips it")
            } else {
                format!(
                    "the text option '{key}' needs '<longest> \"<default>\"' with 1 <= longest <= {TEXT_MAX}; the game skips it"
                )
            };
            let Some((len, rest)) = long_prefix(value) else {
                return Err(bad(need));
            };
            // a text may end with the word `word`: one word, no space key
            let r = rest.trim_end();
            let rest = match r.strip_suffix("word") {
                Some(before) if !secret && (before.is_empty() || before.ends_with(is_space)) => {
                    o.word = true;
                    before
                }
                _ => rest,
            };
            let def = next_token(rest, 0);
            let extra = def.map_or(rest, |(_, end)| &rest[end..]);
            if !(1..=i64::from(TEXT_MAX)).contains(&len)
                || !extra.trim().is_empty()
                || (secret && def.is_some())
                || def.is_some_and(|(d, _)| d.len() as i64 > len)
            {
                return Err(bad(need));
            }
            o.kind = if secret {
                OptionKind::Secret
            } else {
                OptionKind::Text
            };
            o.max = len as i32;
            o.text = def.map_or(String::new(), |(d, _)| d.to_string());
        }
        other => {
            return Err((
                kind_from,
                kind_to,
                format!(
                    "the option '{key}' has the unknown kind '{other}' (bool, choice, int, text or secret); the game skips it"
                ),
            ));
        }
    }
    Ok(Some(o))
}

fn bad_key(whole: (usize, usize), key: &str) -> LineError {
    (
        whole.0,
        whole.1,
        format!(
            "the option key '{key}' is empty, has a space, '=', '#' or a quote, or is over {KEY_MAX} characters; the game ignores it"
        ),
    )
}

/// The pieces of a file, in order.
enum Piece {
    Raw(String),
    Slot,
}

/// Splits `text` into raw lines and entry slots. The comment lines right
/// above an entry are its `lead`.
fn pieces(text: &str) -> (Vec<Piece>, Vec<OptionDecl>, Vec<Diagnostic>) {
    let mut out: Vec<Piece> = Vec::new();
    let mut opts: Vec<OptionDecl> = Vec::new();
    let mut diags: Vec<Diagnostic> = Vec::new();
    let mut pending: Vec<String> = Vec::new();
    let flush = |pending: &mut Vec<String>, out: &mut Vec<Piece>| {
        out.extend(pending.drain(..).map(Piece::Raw));
    };
    for (i, line) in text.lines().enumerate() {
        let n = i + 1;
        if line.trim_start().starts_with('#') {
            pending.push(line.to_string());
            continue;
        }
        match parse_line(line) {
            Ok(Some(mut o)) => {
                o.line = n as u32;
                o.lead = std::mem::take(&mut pending);
                opts.push(o);
                out.push(Piece::Slot);
            }
            Ok(None) => {
                flush(&mut pending, &mut out);
                out.push(Piece::Raw(line.to_string()));
            }
            Err((a, b, msg)) => {
                flush(&mut pending, &mut out);
                out.push(Piece::Raw(line.to_string()));
                diags.push(Diagnostic::at(n, line, a, b, Severity::Error, &msg));
            }
        }
    }
    flush(&mut pending, &mut out);
    (out, opts, diags)
}

/// Reads an options file. The problems are the port's refusals of a line,
/// then the checks of [`check_options`].
pub fn parse_options(text: &str) -> Parsed {
    let (_, options, mut diagnostics) = pieces(text);
    diagnostics.extend(check_options(&options));
    Parsed {
        options,
        diagnostics,
    }
}

// ---------------------------------------------------------------------
// Checking
// ---------------------------------------------------------------------

/// A key a rule can read: words of letters, digits and `_` (not starting
/// with a digit) joined with dots.
pub fn rule_readable(key: &str) -> bool {
    key.split('.').all(|w| {
        !w.is_empty()
            && !w.starts_with(|c: char| c.is_ascii_digit())
            && w.chars().all(|c| c.is_ascii_alphanumeric() || c == '_')
    })
}

/// The key a condition names: `ra.who` of `ra.who!=` or `ra.who=x`.
fn when_key(when: &str) -> &str {
    let eq = when.find('=').unwrap_or(when.len());
    when[..eq].strip_suffix('!').unwrap_or(&when[..eq])
}

/// The problems of a list of entries. A problem of entry `n` (0-based) has
/// the field `option:n`. When the entry comes from a file the diagnostic
/// also has its line.
pub fn check_options(options: &[OptionDecl]) -> Vec<Diagnostic> {
    use Severity::{Error, Warning};
    let mut out = Vec::new();
    let mut at = |i: usize, sev: Severity, msg: &str| {
        let mut d = Diagnostic::field(&format!("option:{i}"), sev, msg);
        d.line = options[i].line;
        out.push(d);
    };
    let keys: BTreeSet<&str> = options
        .iter()
        .filter(|o| o.entry.declares())
        .map(|o| o.key.as_str())
        .collect();
    let mut seen: BTreeSet<&str> = BTreeSet::new();
    let mut option_count = 0;
    for (i, o) in options.iter().enumerate() {
        match o.entry {
            Entry::Heading | Entry::Tab => {
                if o.label.contains('"') {
                    at(i, Error, "the text has no quote");
                } else if o.label.trim().is_empty() {
                    at(i, Warning, &format!("the {} has no text", o.entry.word()));
                }
                if o.entry == Entry::Tab && o.label.len() > TAB_TITLE_MAX {
                    at(
                        i,
                        Warning,
                        &format!(
                            "a tab title is {TAB_TITLE_MAX} characters at most; the game cuts it"
                        ),
                    );
                }
                continue;
            }
            Entry::Help => {
                if !options[..i]
                    .iter()
                    .any(|p| p.entry == Entry::Option && p.key == o.key)
                {
                    at(
                        i,
                        Error,
                        &format!("the help line names '{}', which no option above declares; the game ignores it", o.key),
                    );
                }
                if o.label.contains('"') {
                    at(i, Error, "the text has no quote");
                }
                continue;
            }
            Entry::Option => option_count += 1,
            Entry::Button | Entry::Show => {}
        }
        if option_count > OPTIONS_MAX && o.entry == Entry::Option {
            at(
                i,
                Error,
                &format!("a mod has {OPTIONS_MAX} options at most; the game ignores this one"),
            );
        }
        if o.key.is_empty()
            || o.key
                .contains(|c: char| is_space(c) || matches!(c, '"' | '#' | '='))
        {
            at(
                i,
                Error,
                "a key is one word without spaces, quotes, '#' or '='",
            );
        } else {
            if o.key.len() > KEY_MAX {
                at(
                    i,
                    Error,
                    &format!(
                        "a key is {KEY_MAX} characters at most; the game ignores a longer one"
                    ),
                );
            }
            if o.entry == Entry::Option && o.kind.numeric() && !rule_readable(&o.key) {
                at(
                    i,
                    Warning,
                    "rules read keys of letters, digits and '_' (not starting with a digit); only code can read this one",
                );
            }
            if !seen.insert(&o.key) {
                at(
                    i,
                    Error,
                    "this key is declared before; the game ignores this one",
                );
            }
        }
        if o.label.contains('"') {
            at(i, Error, "a label has no quote");
        } else if o.label.trim().is_empty() {
            at(i, Warning, "the row has no label to show");
        } else if o.label.len() > LABEL_MAX {
            at(
                i,
                Warning,
                &format!("a label is {LABEL_MAX} characters at most; the game cuts it"),
            );
        }
        if !o.when.is_empty() {
            let k = when_key(&o.when);
            if !o.when.contains('=') || k.is_empty() {
                at(i, Error, "a condition is ?key=value or ?key!=value");
            } else if !keys.contains(k) {
                at(
                    i,
                    Warning,
                    &format!("no option or show row is called '{k}', so the row always shows"),
                );
            }
        }
        let lobby = o.tags.split(',').any(|t| t == "lobby");
        if lobby && !(o.entry == Entry::Option && o.kind.numeric()) {
            at(
                i,
                Error,
                "only a bool, choice or int option can be a lobby rule (@lobby)",
            );
        }
        if o.entry != Entry::Option {
            continue;
        }
        match o.kind {
            OptionKind::Bool => {
                if !(0..=1).contains(&o.default) {
                    at(i, Error, "a bool is on or off");
                }
            }
            OptionKind::Choice => {
                if o.values.is_empty() {
                    at(i, Error, "a choice names at least one value");
                }
                if o.values.len() > CHOICES_MAX {
                    at(
                        i,
                        Warning,
                        &format!(
                            "a choice has {CHOICES_MAX} names at most; the game drops the rest"
                        ),
                    );
                }
                for v in &o.values {
                    if v.trim().is_empty() || v.contains(['|', '#', '"']) {
                        at(i, Error, "a name is text without '|', '#' or quotes");
                    } else if v.len() > CHOICE_LEN_MAX {
                        at(
                            i,
                            Warning,
                            &format!(
                                "a name is {CHOICE_LEN_MAX} characters at most; the game cuts '{v}'"
                            ),
                        );
                    }
                }
            }
            OptionKind::Int => {
                let limit = INT_LIMIT;
                if o.min > o.max {
                    at(i, Error, "the lowest value is above the highest");
                } else if o.default < o.min || o.default > o.max {
                    at(
                        i,
                        Error,
                        "the default is outside the lowest and highest value",
                    );
                }
                if o.min < -limit || o.max > limit {
                    at(i, Error, "an int is within a billion of zero");
                }
            }
            OptionKind::Text | OptionKind::Secret => {
                if o.max < 1 || o.max as u32 > TEXT_MAX {
                    at(
                        i,
                        Error,
                        &format!("the longest length is from 1 to {TEXT_MAX}"),
                    );
                }
                if o.kind == OptionKind::Secret && !o.text.is_empty() {
                    at(i, Error, "a secret has no default");
                }
                if o.text.contains('"') || o.text.contains(['\n', '\r']) {
                    at(i, Error, "a text's default has no quote or line break");
                } else if o.text.len() > o.max.max(0) as usize {
                    at(i, Error, "the default is longer than the longest length");
                }
                if o.word && o.text.contains(is_space) {
                    at(i, Error, "a one-word text's default has no space");
                }
            }
        }
        if o.word && o.kind != OptionKind::Text {
            at(i, Error, "only a text option is one word");
        }
    }
    out
}

// ---------------------------------------------------------------------
// Writing
// ---------------------------------------------------------------------

fn same(a: &OptionDecl, b: &OptionDecl) -> bool {
    a.entry == b.entry
        && a.key == b.key
        && a.label == b.label
        && a.kind == b.kind
        && a.default == b.default
        && a.min == b.min
        && a.max == b.max
        && a.values == b.values
        && a.text == b.text
        && a.word == b.word
        && a.when == b.when
        && a.tags == b.tags
}

fn value_text(o: &OptionDecl) -> String {
    match o.kind {
        OptionKind::Bool => (if o.default != 0 { "on" } else { "off" }).to_string(),
        OptionKind::Choice => o.values.join("|"),
        OptionKind::Int => format!("{} {} {}", o.min, o.max, o.default),
        OptionKind::Text => format!(
            "{} \"{}\"{}",
            o.max,
            o.text,
            if o.word { " word" } else { "" }
        ),
        OptionKind::Secret => o.max.to_string(),
    }
}

/// ` ?when @tags`, as the line ends.
fn ends_text(o: &OptionDecl) -> String {
    let mut s = String::new();
    if !o.when.is_empty() {
        s.push_str(&format!(" ?{}", o.when));
    }
    if !o.tags.is_empty() {
        s.push_str(&format!(" @{}", o.tags));
    }
    s
}

/// The text of one line, the option columns padded to `widths` (key, label).
fn render_line(o: &OptionDecl, widths: (usize, usize)) -> String {
    let label = format!("\"{}\"", o.label);
    let mut s = match o.entry {
        Entry::Option => format!(
            "option {:<kw$}  {:<6}  {:<lw$}  {}{}",
            o.key,
            o.kind.word(),
            label,
            value_text(o),
            ends_text(o),
            kw = widths.0,
            lw = widths.1,
        ),
        Entry::Heading | Entry::Tab => format!("{} {label}", o.entry.word()),
        Entry::Help => format!("help {} {label}", o.key),
        Entry::Button | Entry::Show => {
            format!("{} {} {label}{}", o.entry.word(), o.key, ends_text(o))
        }
    };
    if !o.comment.trim().is_empty() {
        s.push_str("  ");
        s.push_str(o.comment.trim());
    }
    s.trim_end().to_string()
}

/// `text` with its entries replaced by `options`, in that order. Comments,
/// blank lines and lines the editor keeps (`mode`, `yaml`, lines the port
/// skips) stay where they are; the comment lines above an entry move with
/// it; a line nobody edited stays as it is. The entries fill the places of
/// the old ones in order, and extra ones go after the last.
pub fn render_options(text: &str, options: &[OptionDecl]) -> String {
    let crlf = text.contains("\r\n");
    let (parts, _, _) = pieces(text);
    let opts = options.iter().filter(|o| o.entry == Entry::Option);
    let widths = (
        opts.clone().map(|o| o.key.len()).max().unwrap_or(0),
        opts.map(|o| o.label.len() + 2).max().unwrap_or(0),
    );
    let line_of = |o: &OptionDecl, out: &mut Vec<String>| {
        out.extend(o.lead.iter().cloned());
        let keep = !o.raw.is_empty() && matches!(parse_line(&o.raw), Ok(Some(p)) if same(&p, o));
        out.push(if keep {
            o.raw.clone()
        } else {
            render_line(o, widths)
        });
    };
    let mut lines: Vec<String> = Vec::new();
    let mut next = options.iter();
    let last_slot = parts.iter().rposition(|p| matches!(p, Piece::Slot));
    for (i, p) in parts.iter().enumerate() {
        match p {
            Piece::Raw(l) => lines.push(l.clone()),
            Piece::Slot => {
                if let Some(o) = next.next() {
                    line_of(o, &mut lines);
                }
                if Some(i) == last_slot {
                    for o in next.by_ref() {
                        line_of(o, &mut lines);
                    }
                }
            }
        }
    }
    for o in next {
        line_of(o, &mut lines);
    }
    let nl = if crlf { "\r\n" } else { "\n" };
    let mut out = lines.join(nl);
    if !out.is_empty() {
        out.push_str(nl);
    }
    out
}

// ---------------------------------------------------------------------
// The mod
// ---------------------------------------------------------------------

/// The options of the open mod.
#[derive(Debug, Clone, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct OptionsFile {
    /// The manifest's `settings` path, `None` when it names none.
    pub path: Option<String>,
    pub exists: bool,
    pub options: Vec<OptionDecl>,
    pub diagnostics: Vec<Diagnostic>,
}

fn bad_input(msg: &str) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidInput, msg)
}

/// The `settings` path of the manifest (the last line wins).
fn settings_path(project: &Folder) -> Option<String> {
    project
        .manifest
        .other
        .iter()
        .filter_map(|l| {
            let body = l.split('#').next().unwrap_or("");
            let (k, v) = body.split_once('=')?;
            (k.trim() == "settings").then(|| v.trim().to_string())
        })
        .rfind(|v| !v.is_empty())
}

fn inside(rel: &str) -> io::Result<String> {
    use std::path::Component;
    let p = Path::new(rel);
    let ok = !rel.is_empty()
        && !rel.contains(':')
        && p.components()
            .all(|c| matches!(c, Component::Normal(_) | Component::CurDir));
    if ok {
        Ok(rel.replace('\\', "/"))
    } else {
        Err(bad_input(
            "the options file is a path inside the mod folder",
        ))
    }
}

/// Reads the mod's options file; a mod with no `settings` line, or a missing
/// file, has no options.
pub fn read_options_file(root: &Path) -> io::Result<OptionsFile> {
    let project = Folder::open(root)?;
    let Some(path) = settings_path(&project) else {
        return Ok(OptionsFile::default());
    };
    let text = inside(&path).and_then(|rel| fs::read_to_string(root.join(rel)));
    Ok(match text {
        Ok(text) => {
            let parsed = parse_options(&text);
            OptionsFile {
                path: Some(path),
                exists: true,
                options: parsed.options,
                diagnostics: parsed.diagnostics,
            }
        }
        Err(_) => OptionsFile {
            path: Some(path),
            ..OptionsFile::default()
        },
    })
}

/// Writes the options into the mod's options file. A mod with no `settings`
/// line gets `options.txt` and the line. A list the port would skip a line of
/// is refused.
pub fn write_options_file(root: &Path, options: &[OptionDecl]) -> io::Result<OptionsFile> {
    if let Some(d) = check_options(options)
        .into_iter()
        .find(|d| d.severity == Severity::Error)
    {
        return Err(bad_input(&d.message));
    }
    let mut project = Folder::open(root)?;
    let rel = match settings_path(&project) {
        Some(p) => inside(&p)?,
        None => {
            project
                .manifest
                .other
                .push(format!("settings = {OPTIONS_FILE}"));
            project.save_meta()?;
            OPTIONS_FILE.to_string()
        }
    };
    let file = root.join(&rel);
    let old = fs::read_to_string(&file).unwrap_or_default();
    let text = render_options(&old, options);
    if let Some(dir) = file.parent() {
        fs::create_dir_all(dir)?;
    }
    let tmp = file.with_extension("tmp");
    fs::write(&tmp, text)?;
    fs::rename(&tmp, &file)?;
    read_options_file(root)
}

/// What a rule check needs of the mod's options.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SettingsCtx {
    /// The mod has a namespace, so it has settings.
    pub namespace: bool,
    /// The keys of the bool, choice and int options the port loads: the
    /// ones a rule reads.
    pub keys: BTreeSet<String>,
}

/// The settings a rule of the mod may read. The port gives every mod a
/// namespace (its `state` name, or one made from the mod name, or the folder
/// name), so a mod has none only when all three are empty.
pub fn settings_ctx(root: &Path) -> io::Result<SettingsCtx> {
    let project = Folder::open(root)?;
    let file = read_options_file(root)?;
    let state = crate::modcode::read_settings(root)?.state;
    let named = state.is_some_and(|s| !s.trim().is_empty())
        || !project.manifest.name.trim().is_empty()
        || root.file_name().is_some();
    Ok(SettingsCtx {
        namespace: named,
        keys: file
            .options
            .into_iter()
            .filter(|o| o.entry == Entry::Option)
            .take(OPTIONS_MAX)
            .filter(|o| o.kind.numeric())
            .map(|o| o.key)
            .collect(),
    })
}
