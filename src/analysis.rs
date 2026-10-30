use std::collections::HashMap;

use crate::models::{Term, Video};

const STOPWORDS: [&str; 102] = [
    "a", "al", "algo", "ante", "antes", "aqui", "asi", "aun", "bajo", "bien", "cada", "como",
    "con", "contra", "cual", "cuando", "de", "del", "desde", "donde", "dos", "el", "ella", "ellas",
    "ellos", "en", "entre", "era", "es", "esa", "ese", "eso", "esta", "este", "esto", "estos",
    "fue", "ha", "hasta", "hay", "la", "las", "le", "les", "lo", "los", "mas", "me", "mi", "mis",
    "mucho", "muy", "no", "nos", "o", "os", "otra", "otro", "para", "pero", "poco", "por", "porque",
    "que", "quien", "se", "ser", "si", "sin", "sobre", "son", "su", "sus", "tal", "tambien", "te",
    "tiene", "todo", "todos", "tu", "tus", "un", "una", "unas", "uno", "unos", "y", "ya", "yo",
    "the", "and", "for", "with", "this", "that", "from", "you", "your", "how", "what", "why", "new",
];

pub fn normalize(value: &str) -> String {
    value
        .chars()
        .map(|ch| match ch {
            'á' | 'à' | 'ä' | 'â' => 'a',
            'é' | 'è' | 'ë' | 'ê' => 'e',
            'í' | 'ì' | 'ï' | 'î' => 'i',
            'ó' | 'ò' | 'ö' | 'ô' => 'o',
            'ú' | 'ù' | 'ü' | 'û' => 'u',
            'ñ' => 'n',
            'ç' => 'c',
            other => other,
        })
        .flat_map(char::to_lowercase)
        .collect()
}

pub fn mean(values: &[f64]) -> Option<f64> {
    let usable: Vec<f64> = values.iter().copied().filter(|v| v.is_finite()).collect();
    if usable.is_empty() {
        None
    } else {
        Some(usable.iter().sum::<f64>() / usable.len() as f64)
    }
}

pub fn median(values: &[f64]) -> Option<f64> {
    let mut usable: Vec<f64> = values.iter().copied().filter(|v| v.is_finite()).collect();
    if usable.is_empty() {
        return None;
    }
    usable.sort_by(f64::total_cmp);
    let middle = usable.len() / 2;
    if usable.len() % 2 == 0 {
        Some((usable[middle - 1] + usable[middle]) / 2.0)
    } else {
        Some(usable[middle])
    }
}

pub fn clamp01(value: f64) -> f64 {
    value.clamp(0.0, 1.0)
}

pub fn log_scale(value: f64, reference: f64) -> f64 {
    if value <= 0.0 || reference <= 1.0 {
        return 0.0;
    }
    clamp01((value.ln_1p()) / reference.ln_1p())
}

fn is_stopword(word: &str) -> bool {
    STOPWORDS.contains(&word)
}

pub fn extract_terms(titles: &[String], limit: usize) -> Vec<Term> {
    let mut counts: HashMap<String, (String, u32)> = HashMap::new();
    for title in titles {
        let mut seen: Vec<String> = Vec::new();
        for raw in title.split(|ch: char| !ch.is_alphanumeric()) {
            let word = normalize(raw);
            if word.chars().count() < 3
                || word.chars().all(|ch| ch.is_numeric())
                || is_stopword(&word)
                || seen.contains(&word)
            {
                continue;
            }
            seen.push(word.clone());
            let entry = counts.entry(word).or_insert_with(|| (raw.to_lowercase(), 0));
            entry.1 += 1;
        }
    }
    let mut terms: Vec<Term> = counts
        .into_values()
        .filter(|(_, count)| *count >= 2)
        .map(|(term, count)| Term { term, count })
        .collect();
    terms.sort_by(|a, b| b.count.cmp(&a.count).then_with(|| a.term.cmp(&b.term)));
    terms.truncate(limit);
    terms
}

pub fn extract_hashtags(descriptions: &[String], limit: usize) -> Vec<Term> {
    let mut counts: HashMap<String, (String, u32)> = HashMap::new();
    for description in descriptions {
        let mut found: Vec<String> = Vec::new();
        let mut current = String::new();
        for ch in description.chars() {
            if ch == '#' {
                current.clear();
                current.push('#');
            } else if !current.is_empty() && (ch.is_alphanumeric() || ch == '_') {
                current.push(ch);
            } else if !current.is_empty() {
                if current.chars().count() > 2 {
                    let key = normalize(&current);
                    if !found.contains(&key) {
                        found.push(key.clone());
                        let entry = counts
                            .entry(key)
                            .or_insert_with(|| (current.to_lowercase(), 0));
                        entry.1 += 1;
                    }
                }
                current.clear();
            }
        }
        if current.chars().count() > 2 {
            let key = normalize(&current);
            if !found.contains(&key) {
                let entry = counts
                    .entry(key)
                    .or_insert_with(|| (current.to_lowercase(), 0));
                entry.1 += 1;
            }
        }
    }
    let mut tags: Vec<Term> = counts
        .into_values()
        .map(|(term, count)| Term { term, count })
        .collect();
    tags.sort_by(|a, b| b.count.cmp(&a.count).then_with(|| a.term.cmp(&b.term)));
    tags.truncate(limit);
    tags
}

pub fn avg_views(videos: &[Video]) -> Option<f64> {
    if videos.is_empty() {
        return None;
    }
    Some(videos.iter().map(|v| v.views as f64).sum::<f64>() / videos.len() as f64)
}

pub fn median_engagement(videos: &[Video]) -> Option<f64> {
    let values: Vec<f64> = videos.iter().filter_map(Video::engagement).collect();
    median(&values)
}

pub fn uploads_per_week(times_ms: &[f64]) -> f64 {
    let mut usable: Vec<f64> = times_ms.iter().copied().filter(|v| v.is_finite()).collect();
    if usable.len() < 2 {
        return 0.0;
    }
    usable.sort_by(f64::total_cmp);
    let span_weeks = (usable[usable.len() - 1] - usable[0]) / 604_800_000.0;
    if span_weeks <= 0.0 {
        0.0
    } else {
        (usable.len() - 1) as f64 / span_weeks
    }
}

pub fn low_view_share(videos: &[Video], threshold: u64) -> f64 {
    if videos.is_empty() {
        return 0.0;
    }
    let low = videos.iter().filter(|v| v.views < threshold).count();
    low as f64 / videos.len() as f64
}

pub fn top_average(videos: &[Video], count: usize) -> f64 {
    let mut views: Vec<f64> = videos.iter().map(|v| v.views as f64).collect();
    views.sort_by(|a, b| b.total_cmp(a));
    let top: Vec<f64> = views.into_iter().take(count).collect();
    mean(&top).unwrap_or(0.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn video(id: &str, views: u64, likes: Option<u64>, comments: Option<u64>) -> Video {
        Video {
            id: id.into(),
            views,
            likes,
            comments,
            ..Video::default()
        }
    }

    #[test]
    fn normalization_strips_accents_and_case() {
        assert_eq!(normalize("Edición EN Español"), "edicion en espanol");
    }

    #[test]
    fn median_handles_even_and_odd() {
        assert_eq!(median(&[3.0, 1.0, 2.0]), Some(2.0));
        assert_eq!(median(&[4.0, 1.0, 3.0, 2.0]), Some(2.5));
        assert_eq!(median(&[]), None);
    }

    #[test]
    fn repeated_title_terms_are_ranked() {
        let titles = vec![
            "Valorant ranked: el clutch imposible".to_string(),
            "Valorant ranked: cómo subir de rango".to_string(),
            "Clutch 1v3 en Valorant".to_string(),
        ];
        let terms = extract_terms(&titles, 5);
        assert_eq!(terms[0].term, "valorant");
        assert_eq!(terms[0].count, 3);
        assert!(terms.iter().any(|t| t.term == "ranked" && t.count == 2));
    }

    #[test]
    fn hashtags_are_counted_once_per_description() {
        let descriptions = vec![
            "Sígueme #Valorant #clips #Valorant".to_string(),
            "Otro día #valorant".to_string(),
        ];
        let tags = extract_hashtags(&descriptions, 5);
        assert_eq!(tags[0].term, "#valorant");
        assert_eq!(tags[0].count, 2);
    }

    #[test]
    fn engagement_ignores_videos_without_signals() {
        let videos = vec![
            video("a", 1000, Some(40), Some(10)),
            video("b", 500, Some(10), None),
            video("c", 100, None, None),
        ];
        assert_eq!(median_engagement(&videos), Some(0.035));
        assert_eq!(avg_views(&videos), Some(1600.0 / 3.0));
    }

    #[test]
    fn cadence_is_weekly_average() {
        let day = 86_400_000.0;
        assert_eq!(uploads_per_week(&[0.0, day * 7.0]), 1.0);
        assert_eq!(uploads_per_week(&[0.0]), 0.0);
    }

    #[test]
    fn low_share_counts_small_videos() {
        let videos = vec![video("a", 50_000, None, None), video("b", 500_000, None, None)];
        assert_eq!(low_view_share(&videos, 100_000), 0.5);
    }
}
