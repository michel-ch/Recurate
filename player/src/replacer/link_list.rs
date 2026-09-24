//! Turn a pasted block of text (one entry per line) into download intents.
//! A line that parses as a YouTube URL is a link; anything else is a free
//! text title such as `Powfu death bed`.

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum LineItem {
    Link(String),
    Title(String),
}

impl LineItem {
    pub fn text(&self) -> &str {
        match self {
            LineItem::Link(s) | LineItem::Title(s) => s,
        }
    }
}

pub fn parse_lines(text: &str) -> Vec<LineItem> {
    let mut seen = std::collections::HashSet::new();
    let mut out = Vec::new();
    for raw in text.lines() {
        let line = raw.trim();
        if line.is_empty() {
            continue;
        }
        let item = if is_youtube_url(line) {
            let url = if line.starts_with("http://") || line.starts_with("https://") {
                line.to_string()
            } else {
                format!("https://{line}")
            };
            LineItem::Link(url)
        } else {
            LineItem::Title(line.to_string())
        };
        if seen.insert(item.clone()) {
            out.push(item);
        }
    }
    out
}

pub fn is_youtube_url(s: &str) -> bool {
    let s = s.trim().to_ascii_lowercase();
    let rest = s
        .strip_prefix("https://")
        .or_else(|| s.strip_prefix("http://"))
        .unwrap_or(&s);
    let host = rest.split('/').next().unwrap_or("");
    matches!(
        host,
        "youtube.com" | "www.youtube.com" | "m.youtube.com" | "music.youtube.com" | "youtu.be"
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn classifies_links_and_titles_and_skips_blanks() {
        let text = "https://www.youtube.com/watch?v=abc123\n\n  Powfu death bed  \nyoutu.be/xyz\n";
        assert_eq!(
            parse_lines(text),
            vec![
                LineItem::Link("https://www.youtube.com/watch?v=abc123".into()),
                LineItem::Title("Powfu death bed".into()),
                LineItem::Link("https://youtu.be/xyz".into()),
            ]
        );
    }

    #[test]
    fn recognises_youtube_hosts_only() {
        assert!(is_youtube_url("https://music.youtube.com/watch?v=a"));
        assert!(is_youtube_url("https://www.youtube.com/playlist?list=PL1"));
        assert!(is_youtube_url("https://youtube.com/shorts/a"));
        assert!(!is_youtube_url("https://example.com/watch?v=a"));
        assert!(!is_youtube_url("Powfu - death bed"));
    }

    #[test]
    fn dedupes_identical_lines() {
        let text = "a song\na song\nhttps://youtu.be/x\nhttps://youtu.be/x";
        assert_eq!(parse_lines(text).len(), 2);
    }
}
