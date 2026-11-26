//! Match a track from another catalogue (Spotify) to a YouTube Music song, so it can play from
//! YouTube Music. Pure scoring, no network. The rules come from the `catalog_match_lab` example
//! measured for the Spotify import: title, artists and duration must all agree, explicit `feat.`
//! credits may stand in for secondary artists, and a different version (live, remix, sped up, …)
//! is never accepted as the original.

use std::collections::HashSet;

/// The track to find.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MatchSource {
    pub title: String,
    pub artists: Vec<String>,
    pub duration_ms: Option<u64>,
}

/// One YouTube Music result.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MatchCandidate {
    pub id: String,
    pub title: String,
    pub artists: Vec<String>,
    pub duration_ms: Option<u64>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Score {
    pub value: f64,
    pub eligible: bool,
    pub reasons: Vec<&'static str>,
}

fn words(value: &str) -> String {
    value
        .to_lowercase()
        .chars()
        .map(|character| if character.is_alphanumeric() { character } else { ' ' })
        .collect::<String>()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

fn version_markers(title: &str) -> HashSet<&'static str> {
    let normalized = words(title);
    let tokens: HashSet<_> = normalized.split_whitespace().collect();
    let mut markers = HashSet::new();
    for marker in ["remix", "cover", "instrumental", "karaoke", "acoustic", "slowed", "nightcore"] {
        if tokens.contains(marker) {
            markers.insert(marker);
        }
    }
    if normalized.contains("sped up") {
        markers.insert("sped_up");
    }
    if normalized.contains("radio edit") {
        markers.insert("radio_edit");
    }
    if normalized.contains("extended") {
        markers.insert("extended");
    }
    let lower = title.to_lowercase();
    if lower.contains("(live") || lower.contains("[live") || lower.contains(" - live") {
        markers.insert("live");
    }
    markers
}

fn is_annotation(value: &str) -> bool {
    let normalized = words(value);
    let tokens: HashSet<_> = normalized.split_whitespace().collect();
    tokens.contains("remaster")
        || tokens.contains("remastered")
        || tokens.contains("feat")
        || tokens.contains("ft")
        || tokens.contains("featuring")
        || !version_markers(value).is_empty()
        || tokens.contains("live")
        || normalized == "official audio"
        || normalized == "official music video"
}

fn base_title(title: &str) -> String {
    let mut result = title.to_lowercase();
    for separator in [" feat.", " feat ", " ft.", " featuring "] {
        if let Some((head, _)) = result.split_once(separator) {
            result = head.to_owned();
        }
    }
    for (open, close) in [('(', ')'), ('[', ']')] {
        while let Some(start) = result.find(open) {
            let Some(relative_end) = result[start..].find(close) else { break };
            let end = start + relative_end + close.len_utf8();
            if !is_annotation(&result[start..end]) {
                break;
            }
            result.replace_range(start..end, " ");
        }
    }
    if let Some((head, suffix)) = result.rsplit_once(" - ") {
        if is_annotation(suffix) {
            result = head.to_owned();
        }
    }
    words(&result)
}

fn annotation_spans(title: &str) -> Vec<(usize, usize)> {
    let mut spans = Vec::new();
    let mut offset = 0;
    while let Some((relative, open)) =
        title[offset..].char_indices().find(|(_, value)| matches!(value, '(' | '['))
    {
        let start = offset + relative;
        let close = if open == '(' { ')' } else { ']' };
        let Some(end) = title[start + 1..].find(close).map(|end| start + 1 + end + 1) else {
            break;
        };
        if !title[start + 1..end - 1].contains(['(', '[']) {
            spans.push((start, end));
        }
        offset = end;
    }
    spans
}

fn collaboration_credit(value: &str) -> Option<&str> {
    let value = value.trim();
    ["feat. ", "feat ", "ft. ", "ft ", "featuring ", "with "].iter().find_map(|prefix| {
        value
            .get(..prefix.len())
            .filter(|head| head.eq_ignore_ascii_case(prefix))
            .map(|_| value[prefix.len()..].trim())
            .filter(|credit| !credit.is_empty())
    })
}

fn explicit_credits(title: &str) -> Vec<&str> {
    let mut credits: Vec<_> = annotation_spans(title)
        .into_iter()
        .filter_map(|(start, end)| collaboration_credit(&title[start + 1..end - 1]))
        .collect();
    for (index, _) in title.char_indices() {
        let suffix = &title[index..];
        for marker in [" feat. ", " feat ", " ft. ", " ft ", " featuring "] {
            if suffix.get(..marker.len()).is_some_and(|head| head.eq_ignore_ascii_case(marker)) {
                let credit = suffix[marker.len()..]
                    .split(" - ")
                    .next()
                    .unwrap_or_default()
                    .split(['(', ')', '[', ']'])
                    .next()
                    .unwrap_or_default()
                    .trim();
                if !credit.is_empty() {
                    credits.push(credit);
                }
            }
        }
    }
    credits
}

fn split_credit(credit: &str) -> Vec<String> {
    credit
        .to_lowercase()
        .replace(" & ", ",")
        .replace(" and ", ",")
        .replace(" x ", ",")
        .split([',', ';'])
        .map(words)
        .collect()
}

fn credit_matches(credit: &str, artist: &str) -> bool {
    let expected = words(artist);
    !expected.is_empty() && (words(credit) == expected || split_credit(credit).contains(&expected))
}

fn credit_is_known(credit: &str, source: &MatchSource) -> bool {
    let known: Vec<_> = source
        .artists
        .iter()
        .map(|artist| words(artist))
        .chain(explicit_credits(&source.title).into_iter().map(words))
        .collect();
    known.contains(&words(credit))
        || split_credit(credit).iter().all(|part| !part.is_empty() && known.contains(part))
}

fn title_for_match(title: &str, source_artists: &[String]) -> String {
    let mut title = title.to_owned();
    for (start, end) in annotation_spans(&title).into_iter().rev() {
        let section = &title[start + 1..end - 1];
        if section.get(..5).is_some_and(|prefix| prefix.eq_ignore_ascii_case("with ")) {
            if let Some(credit) = collaboration_credit(section) {
                if source_artists.iter().any(|artist| words(artist) == words(credit)) {
                    title.replace_range(start..end, " ");
                }
            }
        }
    }
    base_title(&title)
}

fn similarity(left: &str, right: &str) -> f64 {
    if left.is_empty() || right.is_empty() {
        return 0.0;
    }
    let left: Vec<_> = left.chars().collect();
    let right: Vec<_> = right.chars().collect();
    if left.len() > 600 || right.len() > 600 {
        return 0.0;
    }
    let mut previous: Vec<usize> = (0..=right.len()).collect();
    for (i, a) in left.iter().enumerate() {
        let mut current = vec![i + 1; right.len() + 1];
        for (j, b) in right.iter().enumerate() {
            current[j + 1] =
                (previous[j + 1] + 1).min(current[j] + 1).min(previous[j] + usize::from(a != b));
        }
        previous = current;
    }
    1.0 - previous[right.len()] as f64 / left.len().max(right.len()) as f64
}

/// `m:ss` or `h:mm:ss` → milliseconds. Anything else, or zero, is `None` rather than a guess.
pub fn duration_ms(text: &str) -> Option<u64> {
    let parts: Vec<_> = text.split(':').collect();
    if !(2..=3).contains(&parts.len()) {
        return None;
    }
    let mut seconds = 0u64;
    for (index, part) in parts.iter().enumerate() {
        if part.is_empty() || !part.bytes().all(|value| value.is_ascii_digit()) {
            return None;
        }
        let value: u64 = part.parse().ok()?;
        if index > 0 && value >= 60 {
            return None;
        }
        seconds = seconds.checked_mul(60)?.checked_add(value)?;
    }
    seconds.checked_mul(1000).filter(|value| *value > 0)
}

/// Whether the source carries enough to search for and to judge a candidate against.
pub fn searchable(source: &MatchSource) -> bool {
    !base_title(&source.title).is_empty()
        && !source.artists.is_empty()
        && source.artists.iter().all(|artist| !words(artist).is_empty())
        && source.duration_ms.is_some_and(|duration| duration > 0)
}

pub fn score(source: &MatchSource, candidate: &MatchCandidate) -> Score {
    let title = similarity(
        &title_for_match(&source.title, &source.artists),
        &title_for_match(&candidate.title, &source.artists),
    );
    let credits = explicit_credits(&candidate.title);
    let artist = source
        .artists
        .iter()
        .enumerate()
        .map(|(index, primary)| {
            let listed = candidate
                .artists
                .iter()
                .map(|name| similarity(&words(primary), &words(name)))
                .fold(0.0, f64::max);
            if index > 0 && credits.iter().any(|credit| credit_matches(credit, primary)) {
                1.0
            } else {
                listed
            }
        })
        .reduce(f64::min)
        .unwrap_or(0.0);
    let delta = source.duration_ms.zip(candidate.duration_ms).map(|(a, b)| a.abs_diff(b));
    let tolerance =
        source.duration_ms.unwrap_or(0).saturating_mul(4).saturating_div(100).clamp(3000, 10000);
    let mut reasons = Vec::new();
    if title < 0.85 {
        reasons.push("title_mismatch");
    }
    if artist < 0.95 {
        reasons.push("artist_mismatch");
    }
    if credits.iter().any(|credit| !credit_is_known(credit, source)) {
        reasons.push("unverified_collaborator");
    }
    let source_versions = version_markers(&source.title);
    if source_versions != version_markers(&candidate.title)
        || (!source_versions.is_empty() && words(&source.title) != words(&candidate.title))
    {
        reasons.push("version_mismatch");
    }
    match delta {
        Some(value) if value > tolerance => reasons.push("duration_mismatch"),
        None => reasons.push("duration_missing"),
        _ => {}
    }
    let duration =
        delta.map(|value| 1.0 - (value as f64 / tolerance as f64).min(1.0)).unwrap_or(0.0);
    let value = title * 0.55 + artist * 0.30 + duration * 0.15;
    Score { value, eligible: reasons.is_empty(), reasons }
}

/// The best candidate that passes every rule, for playback. Unlike the supervised import, a near
/// tie between two valid candidates still plays the higher one: nobody is there to review it, and
/// both already agree on title, artists, version and duration. `None` when nothing qualifies.
pub fn best_for_playback(source: &MatchSource, candidates: &[MatchCandidate]) -> Option<usize> {
    let mut seen = HashSet::new();
    candidates
        .iter()
        .enumerate()
        .filter(|(_, candidate)| seen.insert(candidate.id.as_str()))
        .map(|(index, candidate)| (index, score(source, candidate)))
        .filter(|(_, score)| score.eligible)
        .max_by(|left, right| left.1.value.total_cmp(&right.1.value))
        .map(|(index, _)| index)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn source() -> MatchSource {
        MatchSource {
            title: "A Song".into(),
            artists: vec!["Example Artist".into()],
            duration_ms: Some(180_000),
        }
    }

    fn candidate(title: &str, artist: &str, duration_ms: Option<u64>) -> MatchCandidate {
        MatchCandidate {
            id: format!("{title}-{artist}"),
            title: title.into(),
            artists: vec![artist.into()],
            duration_ms,
        }
    }

    #[test]
    fn the_same_recording_is_eligible() {
        assert!(score(&source(), &candidate("A Song", "Example Artist", Some(181_000))).eligible);
    }

    #[test]
    fn wrong_artist_title_or_duration_is_rejected() {
        for result in [
            candidate("A Song", "Different Person", Some(180_000)),
            candidate("Another Tune", "Example Artist", Some(180_000)),
            candidate("A Song", "Example Artist", Some(240_000)),
            candidate("A Song", "Example Artist", None),
        ] {
            assert!(!score(&source(), &result).eligible, "{result:?}");
        }
    }

    #[test]
    fn alternate_versions_are_rejected() {
        for title in [
            "A Song (Live)",
            "A Song - Remix",
            "A Song (Cover)",
            "A Song (Instrumental)",
            "A Song (Sped Up)",
            "A Song (Acoustic)",
            "A Song (Radio Edit)",
            "A Song (Slowed)",
        ] {
            assert!(!score(&source(), &candidate(title, "Example Artist", Some(180_000))).eligible);
        }
        assert!(version_markers("Live Forever").is_empty());
    }

    #[test]
    fn different_remixers_are_not_the_same_version() {
        let mut track = source();
        track.title = "A Song (First DJ Remix)".into();
        assert!(
            !score(&track, &candidate("A Song (Second DJ Remix)", "Example Artist", Some(180_000)))
                .eligible
        );
    }

    #[test]
    fn explicit_feature_credit_can_supply_a_secondary_artist() {
        let mut track = source();
        track.artists.push("Guest Singer".into());
        assert!(!score(&track, &candidate("A Song", "Example Artist", Some(180_000))).eligible);
        for title in [
            "A Song (feat. Guest Singer)",
            "A Song [ft. Guest Singer]",
            "A Song (featuring Guest Singer)",
            "A Song feat. Guest Singer",
        ] {
            assert!(score(&track, &candidate(title, "Example Artist", Some(180_000))).eligible);
        }
    }

    #[test]
    fn feature_credits_never_replace_the_primary_artist_or_add_unknown_guests() {
        let mut track = source();
        track.artists.push("Guest Singer".into());
        for result in [
            candidate("A Song (feat. Example Artist)", "Different Person", Some(180_000)),
            candidate("A Song (feat. Guest Singer Junior)", "Example Artist", Some(180_000)),
            candidate("A Song (feat. Guest Singer & Unknown)", "Example Artist", Some(180_000)),
            candidate("A Song (feat. Guest Singer) (Slowed)", "Example Artist", Some(180_000)),
        ] {
            assert!(!score(&track, &result).eligible, "{result:?}");
        }
    }

    #[test]
    fn with_annotations_drop_only_for_a_known_artist() {
        assert_eq!(title_for_match("A Song (With Love)", &source().artists), "a song with love");
        let mut track = source();
        track.title = "A Song (with Guest Singer)".into();
        track.artists.push("Guest Singer".into());
        let mut result = candidate("A Song", "Example Artist", Some(180_000));
        result.artists.push("Guest Singer".into());
        assert!(score(&track, &result).eligible);
    }

    #[test]
    fn durations_parse_strictly() {
        assert_eq!(duration_ms("3:05"), Some(185_000));
        assert_eq!(duration_ms("1:02:03"), Some(3_723_000));
        for value in ["3:99", "live", "", "-1:20", "0:00"] {
            assert_eq!(duration_ms(value), None);
        }
    }

    #[test]
    fn a_source_without_artists_or_duration_is_not_searchable() {
        assert!(searchable(&source()));
        let mut no_artist = source();
        no_artist.artists.clear();
        assert!(!searchable(&no_artist));
        let mut no_duration = source();
        no_duration.duration_ms = None;
        assert!(!searchable(&no_duration));
    }

    #[test]
    fn playback_takes_the_best_valid_candidate_and_never_an_invalid_one() {
        let mut candidates = vec![
            candidate("A Song (Live)", "Example Artist", Some(180_000)),
            candidate("A Song", "Example Artist", Some(186_000)),
            candidate("A Song", "Example Artist", Some(180_500)),
        ];
        candidates[2].id = "closest".into();
        assert_eq!(best_for_playback(&source(), &candidates), Some(2));
        candidates[2].id = candidates[1].id.clone();
        assert_eq!(best_for_playback(&source(), &candidates), Some(1), "a repeated id counts once");
        assert_eq!(best_for_playback(&source(), &candidates[..1]), None);
        assert_eq!(best_for_playback(&source(), &[]), None);
    }
}
