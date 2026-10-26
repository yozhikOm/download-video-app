//! Transcripts: the text of what is spoken in a Video, made from the
//! subtitles or captions the site provides.

use serde::Serialize;

use crate::probe::TranscriptLanguages;

/// A language a Transcript can be saved in, and its source.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct TranscriptLanguage {
    /// The site's code of the language, such as `en` or `pt-BR`.
    pub language: String,
    /// The text comes from captions the site generated rather than from
    /// subtitles the author added.
    pub automatic: bool,
}

/// The suffix YouTube's captions of what is spoken carry among their machine
/// translations into every other language.
const ORIGINAL: &str = "-orig";

/// The languages a Transcript can be saved in: each language of the
/// subtitles, then each language of the captions that has no subtitles.
pub fn languages(available: &TranscriptLanguages) -> Vec<TranscriptLanguage> {
    let manual = available.manual.iter().map(|language| TranscriptLanguage {
        language: language.clone(),
        automatic: false,
    });
    let automatic = captions(available)
        .into_iter()
        .filter(|language| !available.manual.iter().any(|manual| manual == language))
        .map(|language| TranscriptLanguage {
            language: language.into(),
            automatic: true,
        });
    manual.chain(automatic).collect()
}

/// The languages of the captions without machine translations: when the
/// captions name the language spoken (`en-orig`), only it counts.
fn captions(available: &TranscriptLanguages) -> Vec<&str> {
    let originals: Vec<&str> = available
        .automatic
        .iter()
        .filter_map(|language| language.strip_suffix(ORIGINAL))
        .collect();
    if originals.is_empty() {
        available.automatic.iter().map(String::as_str).collect()
    } else {
        originals
    }
}

/// The Transcript language chosen at first: the Video's own, or when it is
/// unknown or has no Transcript, that of the captions, which are of what is
/// spoken; by subtitles over captions and the exact language over a
/// regional variant of it. Otherwise the first of [`languages`].
pub fn default_language(available: &TranscriptLanguages, original: Option<&str>) -> Option<String> {
    let languages = languages(available);
    let in_language = |spoken: &str| {
        languages
            .iter()
            .filter(|offered| primary(&offered.language) == primary(spoken))
            .min_by_key(|offered| (offered.automatic, offered.language != spoken))
    };
    original
        .and_then(in_language)
        .or_else(|| {
            captions(available)
                .first()
                .and_then(|spoken| in_language(spoken))
        })
        .or(languages.first())
        .map(|offered| offered.language.clone())
}

/// The language of a code without its region or script: `en` of `en-GB`.
fn primary(language: &str) -> &str {
    language.split(['-', '_']).next().unwrap_or(language)
}

/// The name yt-dlp knows the subtitles or captions of `language` by among
/// `available`: YouTube's captions of what is spoken go by `en-orig`, which
/// is never a machine translation.
pub fn ytdlp_name(language: &TranscriptLanguage, available: &TranscriptLanguages) -> String {
    let original = format!("{}{ORIGINAL}", language.language);
    if language.automatic && available.automatic.contains(&original) {
        original
    } else {
        language.language.clone()
    }
}

/// The text of WebVTT subtitles: each line of each cue without the
/// timing, tags and cue ids, one per line. A line that repeats the one before
/// it is dropped: YouTube's captions show each line again in the next cue.
pub fn text(subtitles: &str) -> String {
    let mut lines: Vec<String> = Vec::new();
    // The header, comments, styles and cue ids lie outside any cue.
    let mut in_cue = false;
    for line in subtitles.trim_start_matches('\u{feff}').lines() {
        if line.contains("-->") {
            in_cue = true;
        } else if line.is_empty() {
            // Only an empty line ends a cue; one of spaces is its text.
            in_cue = false;
        } else if in_cue {
            let line = decode_entities(&strip_tags(line));
            let line = line.split_whitespace().collect::<Vec<_>>().join(" ");
            if !line.is_empty() && lines.last() != Some(&line) {
                lines.push(line);
            }
        }
    }
    lines.into_iter().map(|line| line + "\n").collect()
}

/// `line` without its `<c>`, `<i>`, `<v Speaker>` and timestamp tags.
fn strip_tags(line: &str) -> String {
    let mut text = String::with_capacity(line.len());
    let mut in_tag = false;
    for c in line.chars() {
        match c {
            '<' => in_tag = true,
            '>' if in_tag => in_tag = false,
            c if !in_tag => text.push(c),
            _ => {}
        }
    }
    text
}

/// `text` with the character references WebVTT and HTML subtitles use
/// replaced by their characters; an unknown one stays as it is.
fn decode_entities(text: &str) -> String {
    let mut decoded = String::with_capacity(text.len());
    let mut rest = text;
    while let Some(at) = rest.find('&') {
        decoded.push_str(&rest[..at]);
        rest = &rest[at..];
        let character = rest.find(';').and_then(|end| {
            let name = &rest[1..end];
            let character = match name {
                "amp" => Some('&'),
                "lt" => Some('<'),
                "gt" => Some('>'),
                "quot" => Some('"'),
                "apos" => Some('\''),
                "nbsp" => Some('\u{a0}'),
                "lrm" => Some('\u{200e}'),
                "rlm" => Some('\u{200f}'),
                _ => name
                    .strip_prefix("#x")
                    .or_else(|| name.strip_prefix("#X"))
                    .map(|hex| u32::from_str_radix(hex, 16))
                    .or_else(|| name.strip_prefix('#').map(str::parse))
                    .and_then(Result::ok)
                    .and_then(char::from_u32),
            };
            Some((character?, end))
        });
        match character {
            Some((character, end)) => {
                decoded.push(character);
                rest = &rest[end + 1..];
            }
            None => {
                decoded.push('&');
                rest = &rest[1..];
            }
        }
    }
    decoded.push_str(rest);
    decoded
}
