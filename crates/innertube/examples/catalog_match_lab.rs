use std::collections::{HashMap, HashSet};
use std::io::{self, Read};
use std::time::Duration;

use innertube::clients::METADATA_CLIENT;
use innertube::{Clients, InnerTube, Session};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Deserialize, Serialize)]
struct Source {
    source: u32,
    position: u32,
    title: String,
    artists: Vec<String>,
    duration_ms: Option<u64>,
    status: String,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
struct Candidate {
    id: String,
    title: String,
    artists: Vec<String>,
    duration_ms: Option<u64>,
}

#[derive(Clone, Debug, Serialize)]
struct Score {
    value: f64,
    title: f64,
    artist: f64,
    duration_delta_ms: Option<u64>,
    eligible: bool,
    reasons: Vec<&'static str>,
}

#[derive(Clone, Debug, Serialize)]
struct ResultRow {
    source: u32,
    position: u32,
    outcome: &'static str,
    completed_search_steps: Option<usize>,
    candidates: usize,
    best: Option<Score>,
    #[serde(skip_serializing_if = "Option::is_none")]
    review: Option<Review>,
}

#[derive(Clone, Debug, Serialize)]
struct ReviewCandidate {
    metadata: Candidate,
    score: Score,
}

#[derive(Clone, Debug, Serialize)]
struct Review {
    original: Source,
    alternatives: Vec<ReviewCandidate>,
}

#[derive(Deserialize)]
struct ReplayCandidate {
    metadata: Candidate,
}

#[derive(Deserialize)]
struct Replay {
    original: Source,
    alternatives: Vec<ReplayCandidate>,
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
            let section = &result[start..end];
            if !is_annotation(section) {
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

fn credit_matches(credit: &str, artist: &str) -> bool {
    let expected = words(artist);
    if expected.is_empty() {
        return false;
    }
    if words(credit) == expected {
        return true;
    }
    credit
        .replace(" & ", ",")
        .replace(" and ", ",")
        .replace(" x ", ",")
        .split([',', ';'])
        .any(|part| words(part) == expected)
}

fn credit_is_known(credit: &str, source: &Source) -> bool {
    let known: Vec<_> = source
        .artists
        .iter()
        .map(|artist| words(artist))
        .chain(explicit_credits(&source.title).into_iter().map(words))
        .collect();
    if known.contains(&words(credit)) {
        return true;
    }
    credit
        .to_lowercase()
        .replace(" & ", ",")
        .replace(" and ", ",")
        .replace(" x ", ",")
        .split([',', ';'])
        .all(|part| !words(part).is_empty() && known.contains(&words(part)))
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

fn duration_ms(text: &str) -> Option<u64> {
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

fn valid_source(source: &Source) -> bool {
    !base_title(&source.title).is_empty()
        && !source.artists.is_empty()
        && source.artists.iter().all(|artist| !words(artist).is_empty())
        && source.duration_ms.is_some_and(|duration| duration > 0)
}

fn score(source: &Source, candidate: &Candidate) -> Score {
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
    Score { value, title, artist, duration_delta_ms: delta, eligible: reasons.is_empty(), reasons }
}

fn rank(source: &Source, candidates: &[Candidate], requests: usize) -> ResultRow {
    rank_with_review(source, candidates, requests, false)
}

fn rank_with_review(
    source: &Source,
    candidates: &[Candidate],
    requests: usize,
    review: bool,
) -> ResultRow {
    let mut seen = HashSet::new();
    let mut scores: Vec<_> = candidates
        .iter()
        .filter(|candidate| seen.insert(candidate.id.as_str()))
        .map(|candidate| (candidate, score(source, candidate)))
        .collect();
    scores.sort_by(|left, right| {
        right.1.eligible.cmp(&left.1.eligible).then_with(|| right.1.value.total_cmp(&left.1.value))
    });
    let outcome = match scores.first().map(|(_, score)| score) {
        Some(best) if best.eligible && best.value >= 0.90 => {
            let ambiguous = scores
                .get(1)
                .is_some_and(|(_, second)| second.eligible && best.value - second.value < 0.04);
            if ambiguous {
                "low_confidence"
            } else {
                "matched"
            }
        }
        Some(best)
            if best.eligible || best.reasons.iter().all(|reason| *reason == "duration_missing") =>
        {
            "low_confidence"
        }
        _ => "not_found",
    };
    ResultRow {
        source: source.source,
        position: source.position,
        outcome,
        completed_search_steps: Some(requests),
        candidates: scores.len(),
        best: scores.first().map(|(_, score)| score.clone()),
        review: review.then(|| Review {
            original: source.clone(),
            alternatives: scores
                .iter()
                .take(5)
                .map(|(candidate, score)| ReviewCandidate {
                    metadata: (*candidate).clone(),
                    score: score.clone(),
                })
                .collect(),
        }),
    }
}

async fn find_candidates(
    tube: &InnerTube,
    client: &innertube::clients::YouTubeClient,
    source: &Source,
    review: bool,
) -> Result<ResultRow, &'static str> {
    let mut candidates = Vec::new();
    let query = format!("{} {}", source.title, source.artists[0]);
    for step in 0..3 {
        if step == 1 {
            let result = tube.search_all(client, &query).await.map_err(|_| "search_error")?;
            candidates.extend(
                result.songs.into_iter().filter(|song| song.kind == "song").take(30).map(|song| {
                    Candidate {
                        id: song.id,
                        title: song.title,
                        artists: song
                            .artist_runs
                            .iter()
                            .filter(|run| run.id.is_some())
                            .map(|run| run.text.clone())
                            .collect(),
                        duration_ms: song.duration.as_deref().and_then(duration_ms),
                    }
                }),
            );
        } else {
            let text = if step == 0 { &query } else { &source.title };
            let result = tube.search_songs(client, text).await.map_err(|_| "search_error")?;
            candidates.extend(result.items.into_iter().take(30).map(|song| Candidate {
                id: song.video_id,
                title: song.title,
                artists: if song.artist_runs.iter().any(|run| run.id.is_some()) {
                    song.artist_runs
                        .iter()
                        .filter(|run| run.id.is_some())
                        .map(|run| run.text.clone())
                        .collect()
                } else {
                    vec![song.artists]
                },
                duration_ms: song.duration.as_deref().and_then(duration_ms),
            }));
        }
        let result = if review {
            rank_with_review(source, &candidates, step + 1, true)
        } else {
            rank(source, &candidates, step + 1)
        };
        if result.outcome == "matched" || step == 2 {
            return Ok(result);
        }
        tokio::time::sleep(Duration::from_millis(500)).await;
    }
    Err("search_error")
}

fn skipped(source: &Source, outcome: &'static str) -> ResultRow {
    ResultRow {
        source: source.source,
        position: source.position,
        outcome,
        completed_search_steps: if outcome == "search_error" { None } else { Some(0) },
        candidates: 0,
        best: None,
        review: None,
    }
}

async fn run() -> Result<(), &'static str> {
    let arguments = std::env::args().skip(1).collect::<Vec<_>>();
    let review = arguments == ["--live", "--review"] || arguments == ["--replay", "--review"];
    let replay = arguments == ["--replay"] || arguments == ["--replay", "--review"];
    if !review && !replay && arguments != ["--live"] {
        return Err(
            "usage: catalog_match_lab --live [--review] or --replay [--review] (JSON on stdin)",
        );
    }
    let mut input = String::new();
    io::stdin().take(524_289).read_to_string(&mut input).map_err(|_| "cannot read input")?;
    if input.len() > 524_288 {
        return Err("input too large");
    }
    if replay {
        let reviews: Vec<Replay> =
            serde_json::from_str(&input).map_err(|_| "invalid replay input")?;
        if reviews.is_empty() || reviews.len() > 30 {
            return Err("provide between 1 and 30 replay entries");
        }
        for entry in reviews {
            if entry.alternatives.len() > 100 {
                return Err("too many replay candidates");
            }
            let candidates: Vec<_> =
                entry.alternatives.into_iter().map(|value| value.metadata).collect();
            let row = if entry.original.status == "unsupported" {
                skipped(&entry.original, "unsupported")
            } else if !valid_source(&entry.original) || entry.original.status != "ready" {
                skipped(&entry.original, "source_error")
            } else {
                rank_with_review(&entry.original, &candidates, 0, review)
            };
            println!("{}", serde_json::to_string(&row).map_err(|_| "cannot encode result")?);
        }
        return Ok(());
    }
    let sources: Vec<Source> = serde_json::from_str(&input).map_err(|_| "invalid source input")?;
    if sources.is_empty() || sources.len() > 30 {
        return Err("provide between 1 and 30 source occurrences");
    }
    if sources.iter().any(|source| {
        source.title.chars().count() > 300
            || source.artists.len() > 30
            || source.artists.iter().any(|artist| artist.chars().count() > 300)
    }) {
        return Err("source metadata exceeds experiment bounds");
    }
    let tube = InnerTube::new(Session::default(), None).map_err(|_| "cannot initialize search")?;
    let clients = Clients::bundled();
    let client = clients.get(METADATA_CLIENT).ok_or("metadata client missing")?;
    let mut cache = HashMap::<String, ResultRow>::new();
    for source in sources {
        let row = if source.status == "unsupported" {
            skipped(&source, "unsupported")
        } else if source.status != "ready" || !valid_source(&source) {
            skipped(&source, "source_error")
        } else {
            let key = serde_json::to_string(&(
                source.title.as_str(),
                &source.artists,
                source.duration_ms,
            ))
            .map_err(|_| "cannot prepare cache key")?;
            if let Some(cached) = cache.get(&key) {
                let mut row = cached.clone();
                row.source = source.source;
                row.position = source.position;
                row.completed_search_steps = Some(0);
                if let Some(review) = row.review.as_mut() {
                    review.original.source = source.source;
                    review.original.position = source.position;
                }
                row
            } else {
                let result = tokio::time::timeout(
                    Duration::from_secs(30),
                    find_candidates(&tube, client, &source, review),
                )
                .await;
                let row = match result {
                    Ok(Ok(row)) => row,
                    Ok(Err(_)) | Err(_) => skipped(&source, "search_error"),
                };
                if row.outcome != "search_error" {
                    cache.insert(key, row.clone());
                }
                tokio::time::sleep(Duration::from_millis(500)).await;
                row
            }
        };
        println!("{}", serde_json::to_string(&row).map_err(|_| "cannot encode result")?);
    }
    Ok(())
}

#[tokio::main]
async fn main() -> std::process::ExitCode {
    match run().await {
        Ok(()) => std::process::ExitCode::SUCCESS,
        Err(message) => {
            eprintln!("{message}");
            std::process::ExitCode::FAILURE
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn source() -> Source {
        Source {
            source: 1,
            position: 1,
            title: "A Song".into(),
            artists: vec!["Example Artist".into()],
            duration_ms: Some(180_000),
            status: "ready".into(),
        }
    }

    fn candidate(title: &str, artist: &str, duration: Option<u64>) -> Candidate {
        Candidate {
            id: "candidate".into(),
            title: title.into(),
            artists: vec![artist.into()],
            duration_ms: duration,
        }
    }

    #[test]
    fn exact_recording_is_provisionally_matched() {
        assert_eq!(
            rank(&source(), &[candidate("A Song", "Example Artist", Some(180_000))], 1).outcome,
            "matched"
        );
    }

    #[test]
    fn review_output_keeps_ranking_and_exposes_the_proposed_video_without_changing_scores() {
        let candidates = [candidate("A Song", "Example Artist", Some(180_000))];
        let plain = rank(&source(), &candidates, 0);
        let detailed = rank_with_review(&source(), &candidates, 0, true);
        assert_eq!(plain.outcome, detailed.outcome);
        assert_eq!(plain.best.unwrap().value, detailed.best.unwrap().value);
        assert_eq!(detailed.review.unwrap().alternatives[0].metadata.id, "candidate");
    }

    #[test]
    fn remaster_annotation_does_not_change_base_title() {
        assert_eq!(base_title("A Song (2024 Remaster)"), "a song");
    }

    #[test]
    fn unicode_letters_are_preserved() {
        assert_eq!(words("ÁRBOL — 東京"), "árbol 東京");
    }

    #[test]
    fn wrong_artist_is_rejected() {
        assert!(
            !score(&source(), &candidate("A Song", "Different Person", Some(180_000))).eligible
        );
    }

    #[test]
    fn wrong_duration_is_rejected() {
        assert!(!score(&source(), &candidate("A Song", "Example Artist", Some(240_000))).eligible);
    }

    #[test]
    fn missing_duration_requires_review() {
        assert_eq!(
            rank(&source(), &[candidate("A Song", "Example Artist", None)], 1).outcome,
            "low_confidence"
        );
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
        ] {
            assert!(!score(&source(), &candidate(title, "Example Artist", Some(180_000))).eligible);
        }
    }

    #[test]
    fn live_in_original_title_is_not_a_live_recording_marker() {
        assert!(version_markers("Live Forever").is_empty());
    }

    #[test]
    fn two_equal_recordings_require_review() {
        let first = candidate("A Song", "Example Artist", Some(180_000));
        let mut second = first.clone();
        second.id = "another".into();
        assert_eq!(rank(&source(), &[first, second], 1).outcome, "low_confidence");
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
    fn missing_collaborator_is_not_automatically_matched() {
        let mut track = source();
        track.artists.push("Guest Singer".into());
        assert!(!score(&track, &candidate("A Song", "Example Artist", Some(180_000))).eligible);
    }

    #[test]
    fn repeated_video_id_is_one_candidate() {
        let first = candidate("A Song", "Example Artist", Some(180_000));
        assert_eq!(rank(&source(), &[first.clone(), first], 1).candidates, 1);
    }

    #[test]
    fn invalid_duration_text_is_not_guessed() {
        for value in ["3:99", "live", "", "-1:20", "0:00"] {
            assert_eq!(duration_ms(value), None);
        }
    }

    #[test]
    fn duration_parser_supports_hours() {
        assert_eq!(duration_ms("1:02:03"), Some(3_723_000));
    }

    #[test]
    fn missing_source_artist_is_not_searchable() {
        let mut track = source();
        track.artists.clear();
        assert!(!valid_source(&track));
    }

    #[test]
    fn default_output_omits_review_metadata() {
        let row = rank(&source(), &[candidate("A Song", "Example Artist", Some(180_000))], 1);
        assert!(!serde_json::to_string(&row).unwrap().contains("Example Artist"));
    }

    #[test]
    fn review_output_retains_candidate_and_source_metadata() {
        let row = rank_with_review(
            &source(),
            &[candidate("A Song", "Example Artist", Some(180_000))],
            1,
            true,
        );
        let review = row.review.unwrap();
        assert_eq!(review.original.title, review.alternatives[0].metadata.title);
    }

    #[test]
    fn explicit_feature_credit_can_supply_secondary_artist() {
        let mut track = source();
        track.artists.push("Guest Singer".into());
        for title in [
            "A Song (feat. Guest Singer)",
            "A Song [ft. Guest Singer]",
            "A Song (featuring Guest Singer)",
            "A Song feat. Guest Singer",
        ] {
            assert_eq!(
                rank(&track, &[candidate(title, "Example Artist", Some(180_000))], 1).outcome,
                "matched"
            );
        }
    }

    #[test]
    fn with_annotation_is_removed_only_for_a_known_artist() {
        let mut track = source();
        track.title = "A Song (with Guest Singer) - from the series Example".into();
        track.artists.push("Guest Singer".into());
        let mut result =
            candidate("A Song (from the series Example)", "Example Artist", Some(180_000));
        result.artists.push("Guest Singer".into());
        assert_eq!(rank(&track, &[result], 1).outcome, "matched");
    }

    #[test]
    fn with_in_a_real_song_title_is_not_discarded() {
        assert_eq!(title_for_match("A Song (With Love)", &source().artists), "a song with love");
    }

    #[test]
    fn feature_credit_does_not_replace_the_primary_artist() {
        assert!(
            !score(
                &source(),
                &candidate("A Song (feat. Example Artist)", "Different Person", Some(180_000))
            )
            .eligible
        );
    }

    #[test]
    fn partial_featured_artist_names_are_rejected() {
        let mut track = source();
        track.artists.push("Guest Singer".into());
        assert!(
            !score(
                &track,
                &candidate("A Song (feat. Guest Singer Junior)", "Example Artist", Some(180_000))
            )
            .eligible
        );
    }

    #[test]
    fn plain_title_mentions_are_not_artist_credits() {
        assert!(explicit_credits("A Song About Guest Singer").is_empty());
    }

    #[test]
    fn guest_credit_does_not_bypass_version_rejection() {
        let mut track = source();
        track.artists.push("Guest Singer".into());
        assert!(
            !score(
                &track,
                &candidate("A Song (feat. Guest Singer) (Slowed)", "Example Artist", Some(180_000))
            )
            .eligible
        );
    }

    #[test]
    fn guest_credit_does_not_bypass_duration_rejection() {
        let mut track = source();
        track.artists.push("Guest Singer".into());
        assert!(
            !score(
                &track,
                &candidate("A Song (feat. Guest Singer)", "Example Artist", Some(240_000))
            )
            .eligible
        );
    }

    #[test]
    fn entirely_different_artists_still_fail_despite_equal_title_and_duration() {
        let mut track = source();
        track.artists = vec!["First Artist".into(), "Second Artist".into(), "Third Artist".into()];
        assert!(!score(&track, &candidate("A Song", "Other Artist", Some(180_000))).eligible);
    }

    #[test]
    fn newly_added_guest_is_not_ignored_when_comparing_titles() {
        assert!(
            !score(
                &source(),
                &candidate("A Song (feat. Unknown Guest)", "Example Artist", Some(180_000))
            )
            .eligible
        );
    }

    #[test]
    fn feature_credit_with_one_unknown_guest_is_rejected() {
        let mut track = source();
        track.artists.push("Guest Singer".into());
        assert!(
            !score(
                &track,
                &candidate(
                    "A Song (feat. Guest Singer & Unknown Guest)",
                    "Example Artist",
                    Some(180_000)
                )
            )
            .eligible
        );
    }
}
