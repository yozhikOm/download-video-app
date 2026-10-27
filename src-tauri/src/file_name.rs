//! The name a Done Download's files are saved under, made from the file
//! name template in the settings.

#[cfg(test)]
mod tests;

use crate::probe::Video;

/// The template the settings start with: the Video's title alone.
pub const DEFAULT_TEMPLATE: &str = "{title}";

/// Longer names are cut, so that the path fits Windows' limit.
const MAX_NAME_CHARS: usize = 150;

/// What a variable the Video lacks takes along of the template's text next
/// to it, along with spaces, such as the " - " of `{channel} - {title}`.
const SEPARATORS: &[char] = &['-', '–', '—', '_', '.', ',', '·'];

/// The saved file's name without its extension: `template` with `{title}`,
/// `{channel}` and `{date}` (`YYYY-MM-DD`) put in, and the characters
/// Windows forbids replaced by lookalikes, as yt-dlp does, or the Video's id
/// when that can't be a file name. Anything else in braces stays as it is.
pub fn file_name(template: &str, video: &Video) -> String {
    let filled = fill(template, video);
    let replaced: String = filled
        .chars()
        .map(|c| match c {
            '"' => '＂',
            '*' => '＊',
            ':' => '：',
            '<' => '＜',
            '>' => '＞',
            '?' => '？',
            '|' => '｜',
            '/' => '⧸',
            '\\' => '⧹',
            c if c.is_control() => ' ',
            c => c,
        })
        .take(MAX_NAME_CHARS)
        .collect();
    // Windows drops a trailing dot or space from a file name.
    let name = replaced
        .trim_start()
        .trim_end_matches(|c: char| c == '.' || c.is_whitespace());
    if name.is_empty() || is_reserved(name) {
        if video.id.is_empty() {
            "video".into()
        } else {
            video.id.clone()
        }
    } else {
        name.into()
    }
}

/// A piece of a template: its own text, or what a variable puts in.
enum Piece<'a> {
    Text(String),
    Value(&'a str),
}

impl Piece<'_> {
    fn is_blank(&self) -> bool {
        match self {
            Self::Text(text) => text.trim().is_empty(),
            Self::Value(value) => value.trim().is_empty(),
        }
    }
}

/// `template` with the variables put in, in one pass, so that a title with
/// `{channel}` in it stays as it is. A variable the Video lacks takes the
/// separators of the template's text before it, or after it when it comes
/// first, but never what another variable put in.
fn fill(template: &str, video: &Video) -> String {
    let date = video.upload_date.as_deref().map(format_date);
    let variables = [
        ("{title}", Some(video.title.as_str())),
        ("{channel}", video.channel.as_deref()),
        ("{date}", date.as_deref()),
    ];
    let mut pieces = Vec::new();
    let mut text = String::new();
    let mut rest = template;
    while let Some(at) = rest.find('{') {
        text += &rest[..at];
        rest = &rest[at..];
        match variables.iter().find(|(name, _)| rest.starts_with(name)) {
            Some((name, value)) => {
                pieces.push(Piece::Text(std::mem::take(&mut text)));
                pieces.push(Piece::Value(value.unwrap_or_default()));
                rest = &rest[name.len()..];
            }
            None => {
                text.push('{');
                rest = &rest[1..];
            }
        }
    }
    text += rest;
    pieces.push(Piece::Text(text));
    let is_separator = |c: char| c.is_whitespace() || SEPARATORS.contains(&c);
    // Every variable sits between two pieces of text.
    for at in (1..pieces.len()).step_by(2) {
        if !pieces[at].is_blank() {
            continue;
        }
        let comes_first = pieces[..at].iter().all(Piece::is_blank);
        let (Piece::Text(before), Piece::Text(after)) = (&pieces[at - 1], &pieces[at + 1]) else {
            unreachable!("text on both sides");
        };
        let trimmed = if comes_first {
            (
                before.clone(),
                after.trim_start_matches(is_separator).into(),
            )
        } else {
            (before.trim_end_matches(is_separator).into(), after.clone())
        };
        (pieces[at - 1], pieces[at + 1]) = (Piece::Text(trimmed.0), Piece::Text(trimmed.1));
    }
    pieces
        .into_iter()
        .map(|piece| match piece {
            Piece::Text(text) => text,
            Piece::Value(value) => value.into(),
        })
        .collect()
}

/// yt-dlp's `YYYYMMDD` as `YYYY-MM-DD`; anything else as it is.
fn format_date(date: &str) -> String {
    if date.len() == 8 && date.chars().all(|c| c.is_ascii_digit()) {
        format!("{}-{}-{}", &date[..4], &date[4..6], &date[6..])
    } else {
        date.into()
    }
}

/// Names that Windows keeps for devices, whatever extension follows them.
fn is_reserved(name: &str) -> bool {
    let stem = name.split('.').next().unwrap_or_default().trim_end();
    let upper = stem.to_ascii_uppercase();
    matches!(upper.as_str(), "CON" | "PRN" | "AUX" | "NUL")
        || ((upper.starts_with("COM") || upper.starts_with("LPT"))
            && upper.len() == 4
            && upper[3..].chars().all(|c| ('1'..='9').contains(&c)))
}
