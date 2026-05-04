use once_cell::sync::Lazy;
use regex::Regex;
use unicode_normalization::UnicodeNormalization;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Confidence {
    High,
    Medium,
    Low,
}

#[derive(Debug, Clone)]
pub struct CleanedTitle {
    pub track_prefix: String,
    pub query: String,
    pub is_remix: bool,
    pub remix_tag: Option<String>,
    pub confidence: Confidence,
}

static OFFICIAL_RE: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"(?i)\s*[\(\[]\s*official\s*(audio|video|music\s*video|lyric\s*video|lyrics?|visualizer)\s*[\)\]]")
        .unwrap()
});
static HQ_RE: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"(?i)\s*[\(\[]\s*(hq|hd|4k|8k|hi[-\s]?res|audio)\s*[\)\]]").unwrap());
static YEAR_RE: Lazy<Regex> = Lazy::new(|| Regex::new(r"\s*\(\s*(19|20)\d{2}\s*\)").unwrap());
static REMASTER_RE: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"(?i)\s*[\(\[\-]?\s*remaster(ed)?(\s+\d{4})?\s*[\)\]]?").unwrap());
static FEAT_RE: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"(?i)\s*[\(\[]\s*(feat\.?|ft\.?|featuring)\s+[^\)\]]+[\)\]]").unwrap());
static REMIX_RE: Lazy<Regex> = Lazy::new(|| {
    Regex::new(
        r"(?i)\(\s*(slowed(\s*\+\s*reverb)?|reverb|sped\s*up|nightcore|slowed)\s*\)",
    )
    .unwrap()
});
static TYPE_BEAT_RE: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r#"(?i)\(?free\)?[^"]*type\s*beat[^"]*[-—]\s*"?(?P<title>[^"]+)"?"#).unwrap()
});
static TRACK_PREFIX_RE: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"^\s*(?P<digits>\d+)\s*-\s+(?P<rest>.+)$").unwrap());
static WS_RE: Lazy<Regex> = Lazy::new(|| Regex::new(r"\s+").unwrap());

pub fn clean_title(filename_stem: &str) -> CleanedTitle {
    let (track_prefix, rest) = split_track_prefix(filename_stem);

    let normalized: String = rest.nfkc().collect();
    let normalized = destylize_math(&normalized);

    let remix_tag = REMIX_RE
        .find(&normalized)
        .map(|m| m.as_str().to_string());
    let is_remix = remix_tag.is_some();

    let mut working = normalized.clone();

    if let Some(caps) = TYPE_BEAT_RE.captures(&working) {
        if let Some(title) = caps.name("title") {
            let mut confidence = Confidence::Low;
            let q = title.as_str().trim().trim_matches('"').to_string();
            let q = WS_RE.replace_all(&q, " ").trim().to_string();
            let q = strip_quotes(&q);
            let _ = &mut confidence;
            return CleanedTitle {
                track_prefix,
                query: q,
                is_remix,
                remix_tag,
                confidence: Confidence::Low,
            };
        }
    }

    let placeholder = "\u{E000}REMIX\u{E000}";
    if let Some(tag) = &remix_tag {
        working = working.replace(tag.as_str(), placeholder);
    }

    working = OFFICIAL_RE.replace_all(&working, "").into_owned();
    working = HQ_RE.replace_all(&working, "").into_owned();
    working = YEAR_RE.replace_all(&working, "").into_owned();
    working = REMASTER_RE.replace_all(&working, "").into_owned();
    working = FEAT_RE.replace_all(&working, "").into_owned();

    if let Some(tag) = &remix_tag {
        working = working.replace(placeholder, tag.as_str());
    }

    let collapsed = WS_RE.replace_all(working.trim(), " ").to_string();
    let query = collapsed.trim().to_string();

    let confidence = if query.is_empty() {
        Confidence::Low
    } else if query.contains(" - ") {
        Confidence::High
    } else {
        Confidence::Medium
    };

    CleanedTitle {
        track_prefix,
        query,
        is_remix,
        remix_tag,
        confidence,
    }
}

fn split_track_prefix(name: &str) -> (String, String) {
    if let Some(caps) = TRACK_PREFIX_RE.captures(name) {
        let digits = caps.name("digits").map(|m| m.as_str()).unwrap_or("");
        let rest = caps.name("rest").map(|m| m.as_str()).unwrap_or("");
        return (format!("{} - ", digits), rest.to_string());
    }
    (String::new(), name.to_string())
}

fn strip_quotes(s: &str) -> String {
    let s = s.trim();
    s.trim_start_matches(|c: char| c == '"' || c == '\'' || c == '\u{201C}')
        .trim_end_matches(|c: char| c == '"' || c == '\'' || c == '\u{201D}')
        .to_string()
}

fn destylize_math(s: &str) -> String {
    s.chars().map(destylize_char).collect()
}

fn destylize_char(c: char) -> char {
    let cp = c as u32;
    let mapped = match cp {
        0x1D400..=0x1D419 => Some(b'A' + (cp - 0x1D400) as u8),
        0x1D41A..=0x1D433 => Some(b'a' + (cp - 0x1D41A) as u8),
        0x1D434..=0x1D44D => Some(b'A' + (cp - 0x1D434) as u8),
        0x1D44E..=0x1D467 => Some(b'a' + (cp - 0x1D44E) as u8),
        0x1D468..=0x1D481 => Some(b'A' + (cp - 0x1D468) as u8),
        0x1D482..=0x1D49B => Some(b'a' + (cp - 0x1D482) as u8),
        0x1D49C..=0x1D4B5 => Some(b'A' + (cp - 0x1D49C) as u8),
        0x1D4B6..=0x1D4CF => Some(b'a' + (cp - 0x1D4B6) as u8),
        0x1D4D0..=0x1D4E9 => Some(b'A' + (cp - 0x1D4D0) as u8),
        0x1D4EA..=0x1D503 => Some(b'a' + (cp - 0x1D4EA) as u8),
        0x1D504..=0x1D51D => Some(b'A' + (cp - 0x1D504) as u8),
        0x1D51E..=0x1D537 => Some(b'a' + (cp - 0x1D51E) as u8),
        0x1D538..=0x1D551 => Some(b'A' + (cp - 0x1D538) as u8),
        0x1D552..=0x1D56B => Some(b'a' + (cp - 0x1D552) as u8),
        0x1D56C..=0x1D585 => Some(b'A' + (cp - 0x1D56C) as u8),
        0x1D586..=0x1D59F => Some(b'a' + (cp - 0x1D586) as u8),
        0x1D5A0..=0x1D5B9 => Some(b'A' + (cp - 0x1D5A0) as u8),
        0x1D5BA..=0x1D5D3 => Some(b'a' + (cp - 0x1D5BA) as u8),
        0x1D5D4..=0x1D5ED => Some(b'A' + (cp - 0x1D5D4) as u8),
        0x1D5EE..=0x1D607 => Some(b'a' + (cp - 0x1D5EE) as u8),
        0x1D608..=0x1D621 => Some(b'A' + (cp - 0x1D608) as u8),
        0x1D622..=0x1D63B => Some(b'a' + (cp - 0x1D622) as u8),
        0x1D63C..=0x1D655 => Some(b'A' + (cp - 0x1D63C) as u8),
        0x1D656..=0x1D66F => Some(b'a' + (cp - 0x1D656) as u8),
        0x1D670..=0x1D689 => Some(b'A' + (cp - 0x1D670) as u8),
        0x1D68A..=0x1D6A3 => Some(b'a' + (cp - 0x1D68A) as u8),
        _ => None,
    };
    mapped.map(|b| b as char).unwrap_or(c)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn captures_track_prefix() {
        let c = clean_title("120 - death bed (coffee for your head) - Powfu");
        assert_eq!(c.track_prefix, "120 - ");
        assert!(c.query.contains("Powfu"));
    }

    #[test]
    fn strips_official_audio() {
        let c = clean_title("Billie Eilish - BIRDS OF A FEATHER (Official Music Video)");
        assert!(!c.query.to_lowercase().contains("official"));
        assert!(c.query.contains("Billie Eilish"));
    }

    #[test]
    fn preserves_slowed_reverb() {
        let c = clean_title("Goth (Slowed + Reverb)");
        assert!(c.is_remix);
        assert!(c.query.to_lowercase().contains("slowed"));
    }

    #[test]
    fn destylizes_math_unicode() {
        let c = clean_title("Justin Bieber - Love Yourself (𝒮𝓁𝑜𝓌𝑒𝒹 𝒟𝑜𝓌𝓃)");
        assert!(c.query.contains("Slowed Down"));
    }

    #[test]
    fn type_beat_low_confidence() {
        let c = clean_title("(free) post punk + darkwave + coldwave type beat - \"bondage\"");
        assert_eq!(c.confidence, Confidence::Low);
        assert_eq!(c.query, "bondage");
    }

    #[test]
    fn strips_feat() {
        let c = clean_title("46 - Sublime Weakness (feat. Mapps & October Child) - AK");
        assert!(!c.query.to_lowercase().contains("feat"));
        assert_eq!(c.track_prefix, "46 - ");
    }
}
