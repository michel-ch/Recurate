use crate::replacer::title_cleaner::CleanedTitle;
use crate::replacer::youtube::YtVideo;

#[derive(Debug, Clone)]
pub struct ScoredResult {
    pub video: YtVideo,
    pub score: i32,
    pub reasons: Vec<&'static str>,
}

pub fn score_results(
    cleaned: &CleanedTitle,
    expected_duration_secs: Option<u64>,
    videos: &[YtVideo],
) -> Vec<ScoredResult> {
    let query_lower = cleaned.query.to_lowercase();
    let remix_tag_lower = cleaned
        .remix_tag
        .as_ref()
        .map(|t| t.to_lowercase())
        .unwrap_or_default();

    let mut out: Vec<ScoredResult> = videos
        .iter()
        .filter(|v| is_audio_candidate(&v.title, &v.channel_or_uploader()))
        .map(|v| {
            let mut score = 0i32;
            let mut reasons: Vec<&'static str> = Vec::new();

            let title_lower = v.title.to_lowercase();
            let channel = v.channel_or_uploader();
            let channel_lower = channel.to_lowercase();

            if channel_lower.ends_with("- topic") || channel_lower.ends_with("- topic ") {
                score += 12;
                reasons.push("topic channel +12");
            }
            if !query_lower.is_empty() && title_lower.contains(&query_lower) {
                score += 5;
                reasons.push("query in title +5");
            }
            if title_lower.contains("official audio")
                || title_lower.contains("[audio]")
                || title_lower.contains("(audio)")
            {
                score += 6;
                reasons.push("audio tag +6");
            }
            if title_lower.contains("official music video")
                || title_lower.contains("official video")
                || title_lower.contains("(music video)")
            {
                score -= 2;
                reasons.push("music video -2");
            }
            if title_lower.contains("lyric video") || title_lower.contains("(lyrics)") {
                score -= 1;
                reasons.push("lyric video -1");
            }
            if title_lower.contains("visualizer") {
                score -= 1;
                reasons.push("visualizer -1");
            }

            if let (Some(expected), Some(got)) = (expected_duration_secs, v.duration_seconds()) {
                let diff = expected.abs_diff(got);
                if diff <= 5 {
                    score += 3;
                    reasons.push("duration ±5s +3");
                } else if diff <= 15 {
                    score += 2;
                    reasons.push("duration ±15s +2");
                } else if diff > 60 {
                    score -= 2;
                    reasons.push("duration off >60s -2");
                }
            }

            if cleaned.is_remix {
                if !remix_tag_lower.is_empty() && title_lower.contains(&remix_tag_lower) {
                    score += 5;
                    reasons.push("remix tag match +5");
                }
            } else {
                if title_lower.contains("cover") {
                    score -= 3;
                    reasons.push("cover -3");
                }
                if title_lower.contains("remix")
                    || title_lower.contains("slowed")
                    || title_lower.contains("sped up")
                {
                    score -= 3;
                    reasons.push("unwanted remix -3");
                }
            }

            if title_lower.contains("mashup") || title_lower.contains(" mix ") {
                score -= 3;
                reasons.push("mashup/mix -3");
            }
            if title_lower.contains("live") || title_lower.contains("concert") {
                score -= 2;
                reasons.push("live -2");
            }

            ScoredResult {
                video: v.clone(),
                score,
                reasons,
            }
        })
        .collect();

    out.sort_by(|a, b| b.score.cmp(&a.score));
    out
}

fn is_audio_candidate(title: &str, channel: &str) -> bool {
    let t = title.to_lowercase();
    let c = channel.to_lowercase();

    if c.ends_with("- topic") || c.ends_with("- topic ") {
        return true;
    }
    if t.contains("official audio")
        || t.contains("[audio]")
        || t.contains("(audio)")
        || t.contains(" audio ")
        || t.ends_with(" audio")
    {
        return true;
    }

    let video_markers = [
        "official music video",
        "official video",
        "music video",
        "(video)",
        "[video]",
        "visualizer",
        "visualiser",
        "lyric video",
        "lyrics video",
        "live performance",
        "live at ",
        "live in ",
        "live on ",
        "(live)",
        "[live]",
        "concert",
        "music clip",
        "performance video",
    ];
    if video_markers.iter().any(|m| t.contains(m)) {
        return false;
    }

    true
}
