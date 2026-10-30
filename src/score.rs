use crate::analysis::normalize;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Platform {
    Youtube,
    Shorts,
    Tiktok,
}

impl Platform {
    pub fn label(self) -> &'static str {
        match self {
            Platform::Youtube => "YouTube",
            Platform::Shorts => "Shorts",
            Platform::Tiktok => "TikTok",
        }
    }

    pub fn limit(self) -> usize {
        match self {
            Platform::Youtube | Platform::Shorts => 100,
            Platform::Tiktok => 150,
        }
    }

    pub fn cut(self) -> usize {
        match self {
            Platform::Youtube => 60,
            Platform::Shorts => 42,
            Platform::Tiktok => 55,
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct Factor {
    pub label: &'static str,
    pub score: u32,
    pub max: u32,
    pub note: String,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Tip {
    pub text: String,
    pub hard: bool,
}

#[derive(Clone, Debug, PartialEq)]
pub struct TitleAnalysis {
    pub total: u32,
    pub verdict: &'static str,
    pub factors: Vec<Factor>,
    pub tips: Vec<Tip>,
    pub chars: usize,
    pub words: usize,
    pub limit: usize,
    pub cut: usize,
}

const POWER_WORDS: [&str; 34] = [
    "secreto", "error", "truco", "guia", "como", "por que", "nunca", "siempre", "nadie", "todos",
    "mejor", "peor", "evita", "deja", "prueba", "cambie", "cambio", "funciona", "brutal", "real",
    "honesto", "verdad", "problema", "solucion", "gratis", "rapido", "facil", "principiante",
    "principiantes", "antes", "despues", "durante", "sin", "con",
];

const SPAM_WORDS: [&str; 12] = [
    "increible",
    "garantizado",
    "garantizada",
    "100%",
    "gratis",
    "hack",
    "magia",
    "ilimitado",
    "definitivo",
    "obligatorio",
    "urgente",
    "exclusivo",
];

const VAGUE_WORDS: [&str; 5] = ["cosas", "algo", "esto", "eso", "cositas"];

fn count_occurrences(haystack: &str, needle: &str) -> usize {
    if needle.is_empty() {
        return 0;
    }
    haystack.match_indices(needle).count()
}

fn has_power_word(text: &str) -> bool {
    POWER_WORDS.iter().any(|word| text.contains(word))
}

fn has_digit(text: &str) -> bool {
    text.chars().any(|ch| ch.is_ascii_digit())
}

fn symbol_count(value: &str) -> usize {
    value
        .chars()
        .filter(|ch| {
            (*ch as u32) >= 0x2190
        })
        .count()
}

fn all_caps_words(value: &str) -> usize {
    value
        .split_whitespace()
        .filter(|word| {
            let letters: Vec<char> = word.chars().filter(|c| c.is_alphabetic()).collect();
            letters.len() > 2 && letters.iter().all(|c| c.is_uppercase())
        })
        .count()
}

pub fn preview_parts(title: &str, cut: usize) -> (String, String) {
    let mut head = String::new();
    let mut rest = String::new();
    for (index, ch) in title.chars().enumerate() {
        if index < cut {
            head.push(ch);
        } else {
            rest.push(ch);
        }
    }
    (head, rest)
}

pub fn analyze(title: &str, keyword: &str, platform: Platform) -> TitleAnalysis {
    let trimmed = title.trim();
    let chars = title.chars().count();
    let words = trimmed.split_whitespace().count();
    let limit = platform.limit();
    let cut = platform.cut();
    let lowered = normalize(trimmed);
    let keyword_norm = normalize(keyword.trim());

    if trimmed.is_empty() {
        return TitleAnalysis {
            total: 0,
            verdict: "Sin título",
            factors: Vec::new(),
            tips: vec![Tip {
                text: "Escribe un título para empezar el análisis.".into(),
                hard: false,
            }],
            chars: 0,
            words: 0,
            limit,
            cut,
        };
    }

    let mut factors = Vec::new();

    let mut length_score: u32 = if chars <= cut {
        20
    } else if chars <= limit {
        14
    } else {
        5
    };
    if words < 3 {
        length_score = length_score.saturating_sub(5);
    }
    if words > 20 {
        length_score = length_score.saturating_sub(4);
    }
    factors.push(Factor {
        label: "Longitud",
        score: length_score,
        max: 20,
        note: if chars > limit {
            format!("{chars} caracteres; el máximo de {} es {limit}.", platform.label())
        } else {
            format!("{chars} caracteres y {words} palabras; el feed corta hacia {cut}.")
        },
    });

    let mut hook = 4;
    if has_digit(&lowered) {
        hook += 6;
    }
    if has_power_word(&lowered) {
        hook += 6;
    }
    if has_power_word(&lowered.chars().take(30).collect::<String>()) {
        hook += 4;
    }
    if lowered.contains("antes") && lowered.contains("despues") {
        hook += 2;
    }
    hook = hook.min(20);
    factors.push(Factor {
        label: "Gancho",
        score: hook,
        max: 20,
        note: if hook >= 16 {
            "Hay una promesa concreta en la primera parte del título.".into()
        } else if hook >= 10 {
            "El gancho existe, pero puede ser más concreto.".into()
        } else {
            "Añade un dato, una cifra o una promesa clara.".into()
        },
    });

    let mut clarity: u32;
    if keyword_norm.is_empty() {
        clarity = if (4..=14).contains(&words) { 16 } else if words == 3 || (15..=20).contains(&words) { 10 } else { 6 };
        if VAGUE_WORDS.iter().any(|word| lowered.split_whitespace().any(|w| w == *word)) {
            clarity = clarity.saturating_sub(4);
        }
    } else {
        let tokens: Vec<&str> = keyword_norm.split_whitespace().filter(|t| !t.is_empty()).collect();
        let found = tokens.iter().filter(|token| lowered.contains(**token)).count();
        clarity = if found == tokens.len() && !tokens.is_empty() {
            20
        } else if found > 0 {
            12
        } else {
            6
        };
        if VAGUE_WORDS.iter().any(|word| lowered.split_whitespace().any(|w| w == *word)) {
            clarity = clarity.saturating_sub(3);
        }
    }
    factors.push(Factor {
        label: "Claridad",
        score: clarity,
        max: 20,
        note: if keyword_norm.is_empty() {
            "Sin keyword objetivo: se valora que el título se entienda solo.".into()
        } else if clarity == 20 {
            format!("La keyword «{}» aparece completa.", keyword.trim())
        } else if clarity >= 12 {
            format!("La keyword «{}» aparece solo en parte.", keyword.trim())
        } else {
            format!("La keyword «{}» no aparece en el título.", keyword.trim())
        },
    });

    let mut curiosity = 4;
    if trimmed.contains('?') || trimmed.contains('¿') {
        curiosity += 6;
    }
    if has_digit(&lowered) {
        curiosity += 4;
    }
    if lowered.contains("como") || lowered.contains("por que") || lowered.contains("que ") {
        curiosity += 4;
    }
    if ["nadie", "secreto", "error", "antes", "despues", "lo que", "estos", "esta"]
        .iter()
        .any(|word| lowered.contains(word))
    {
        curiosity += 4;
    }
    if trimmed.contains('(') || trimmed.contains('[') {
        curiosity += 2;
    }
    curiosity = curiosity.min(20);
    factors.push(Factor {
        label: "Curiosidad",
        score: curiosity,
        max: 20,
        note: if curiosity >= 16 {
            "Abre una pregunta o un contraste que invita a entrar.".into()
        } else {
            "Prueba una pregunta o un contraste («antes / después»).".into()
        },
    });

    let avg_word: f64 = trimmed.split_whitespace().map(|w| w.chars().count()).sum::<usize>() as f64
        / words.max(1) as f64;
    let mut readability = 4u32;
    if avg_word <= 8.0 {
        readability += 4;
    } else if avg_word <= 11.0 {
        readability += 2;
    }
    if all_caps_words(trimmed) == 0 {
        readability += 2;
    }
    if symbol_count(trimmed) <= 2 {
        readability += 2;
    } else {
        readability = readability.saturating_sub(1);
    }
    readability = readability.min(10);
    factors.push(Factor {
        label: "Legibilidad",
        score: readability,
        max: 10,
        note: format!("Media de {:.1} caracteres por palabra.", avg_word),
    });

    let mut antispam = 10i32;
    let spam_hits: Vec<&&str> = SPAM_WORDS.iter().filter(|word| lowered.contains(**word)).collect();
    antispam -= spam_hits.len() as i32 * 3;
    if all_caps_words(trimmed) > 0 {
        antispam -= 3;
    }
    if count_occurrences(trimmed, "!!") > 0
        || count_occurrences(trimmed, "??") > 0
        || count_occurrences(trimmed, "..") > 0
    {
        antispam -= 2;
    }
    if symbol_count(trimmed) >= 3 {
        antispam -= 2;
    }
    let antispam = antispam.clamp(0, 10) as u32;
    factors.push(Factor {
        label: "Señales de spam",
        score: antispam,
        max: 10,
        note: if spam_hits.is_empty() {
            "Sin promesas absolutas ni signos repetidos.".into()
        } else {
            format!("Revisa: {}.", spam_hits.iter().map(|w| **w).collect::<Vec<_>>().join(", "))
        },
    });

    let total: u32 = factors.iter().map(|factor| factor.score).sum();
    let verdict = if total >= 85 {
        "Muy sólido"
    } else if total >= 70 {
        "Bueno"
    } else if total >= 55 {
        "Mejorable"
    } else {
        "Débil"
    };

    let mut tips = Vec::new();
    if chars > limit {
        tips.push(Tip {
            text: format!("Recorta a {limit} caracteres: {} no admite más.", platform.label()),
            hard: true,
        });
    } else if chars > cut {
        tips.push(Tip {
            text: format!("El feed cortará el título hacia el carácter {cut}; deja ahí la idea clave."),
            hard: false,
        });
    }
    if words < 3 {
        tips.push(Tip {
            text: "El título es demasiado corto para explicar de qué va el vídeo.".into(),
            hard: false,
        });
    }
    if !keyword_norm.is_empty() {
        let tokens: Vec<&str> = keyword_norm.split_whitespace().filter(|t| !t.is_empty()).collect();
        if !tokens.is_empty() && !tokens.iter().all(|token| lowered.contains(*token)) {
            tips.push(Tip {
                text: format!("Incluye «{}» tal como lo buscaría tu audiencia.", keyword.trim()),
                hard: false,
            });
        }
    }
    if hook < 12 {
        tips.push(Tip {
            text: "Concreta el gancho: cifra, tiempo o resultado en la primera frase.".into(),
            hard: false,
        });
    }
    if curiosity < 12 {
        tips.push(Tip {
            text: "Plantea una pregunta o un contraste para abrir curiosidad sin prometer de más.".into(),
            hard: false,
        });
    }
    if all_caps_words(trimmed) > 0 {
        tips.push(Tip {
            text: "Evita palabras enteras en MAYÚSCULAS; pierden legibilidad.".into(),
            hard: false,
        });
    }
    if !spam_hits.is_empty() {
        tips.push(Tip {
            text: "Quita promesas absolutas: un título creíble retiene mejor.".into(),
            hard: false,
        });
    }
    if tips.is_empty() {
        tips.push(Tip {
            text: "Sin cambios urgentes: el título cumple las reglas de longitud, claridad y gancho.".into(),
            hard: false,
        });
    }
    tips.truncate(4);

    TitleAnalysis {
        total,
        verdict,
        factors,
        tips,
        chars,
        words,
        limit,
        cut,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_title_scores_zero() {
        let analysis = analyze("   ", "", Platform::Youtube);
        assert_eq!(analysis.total, 0);
        assert_eq!(analysis.factors.len(), 0);
    }

    #[test]
    fn strong_title_beats_weak_title() {
        let strong = analyze(
            "Cómo subí 3 rangos en Valorant ranked en 7 días sin equipo",
            "valorant ranked",
            Platform::Youtube,
        );
        let weak = analyze("cosas", "", Platform::Youtube);
        assert!(strong.total >= 65, "score was {}", strong.total);
        assert!(strong.total > weak.total);
    }

    #[test]
    fn over_limit_is_hard_tip() {
        let long = "a".repeat(120);
        let analysis = analyze(&long, "", Platform::Youtube);
        assert!(analysis.tips.iter().any(|tip| tip.hard));
    }

    #[test]
    fn caps_reduce_antispam() {
        let caps = analyze("ESTO ES INCREÍBLE GARANTIZADO", "", Platform::Youtube);
        let normal = analyze("Esto funciona de verdad", "", Platform::Youtube);
        assert!(caps.total < normal.total);
    }

    #[test]
    fn preview_splits_at_cut() {
        let (head, rest) = preview_parts("abcdef", 4);
        assert_eq!(head, "abcd");
        assert_eq!(rest, "ef");
    }
}
