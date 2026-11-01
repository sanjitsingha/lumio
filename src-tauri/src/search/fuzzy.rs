//! Fuzzy matching with explicit score tiers.
//!
//! Scores (higher is better) for a single query token against a candidate:
//!
//! | tier | example (`query` → candidate)          | score      |
//! |------|----------------------------------------|------------|
//! | exact             | `firefox` → Firefox        | 1000       |
//! | prefix at word end| `visual studio` → Visual Studio Code | 950 |
//! | whole word        | `code` → Visual Studio Code | 920 - 5/word |
//! | prefix            | `fire` → Firefox            | 880-900    |
//! | word prefix       | `stud` → Visual Studio Code | 800 - 8/word |
//! | acronym           | `vsc` → Visual Studio Code  | 760        |
//! | substring         | `fox` → Firefox             | 540-600    |
//! | subsequence       | `frfx` → Firefox            | 100-500    |
//! | one typo          | `firfox` → Firefox          | 280-300    |

/// Lowercases and folds common Latin diacritics (`É` → `e`, `ß` → `ss`).
pub fn fold(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        for lc in c.to_lowercase() {
            match lc {
                'à' | 'á' | 'â' | 'ã' | 'ä' | 'å' | 'ā' | 'ă' | 'ą' => out.push('a'),
                'ç' | 'ć' | 'č' | 'ĉ' | 'ċ' => out.push('c'),
                'ď' | 'đ' => out.push('d'),
                'è' | 'é' | 'ê' | 'ë' | 'ē' | 'ĕ' | 'ė' | 'ę' | 'ě' => out.push('e'),
                'ĝ' | 'ğ' | 'ġ' | 'ģ' => out.push('g'),
                'ì' | 'í' | 'î' | 'ï' | 'ĩ' | 'ī' | 'ĭ' | 'į' | 'ı' => out.push('i'),
                'ł' | 'ľ' | 'ĺ' | 'ļ' => out.push('l'),
                'ñ' | 'ń' | 'ň' | 'ņ' => out.push('n'),
                'ò' | 'ó' | 'ô' | 'õ' | 'ö' | 'ø' | 'ō' | 'ő' => out.push('o'),
                'ŕ' | 'ř' => out.push('r'),
                'ś' | 'š' | 'ş' | 'ș' => out.push('s'),
                'ť' | 'ţ' | 'ț' => out.push('t'),
                'ù' | 'ú' | 'û' | 'ü' | 'ũ' | 'ū' | 'ŭ' | 'ů' | 'ű' | 'ų' => out.push('u'),
                'ý' | 'ÿ' => out.push('y'),
                'ź' | 'ż' | 'ž' => out.push('z'),
                'ß' => out.push_str("ss"),
                'æ' => out.push_str("ae"),
                'œ' => out.push_str("oe"),
                other => out.push(other),
            }
        }
    }
    out
}

fn is_alnum(c: char) -> bool {
    c.is_alphanumeric()
}

/// Indices where a new word starts (after separators and letter/digit changes).
fn word_starts(t: &[char]) -> Vec<usize> {
    let mut starts = Vec::new();
    for i in 0..t.len() {
        if !is_alnum(t[i]) {
            continue;
        }
        let boundary = i == 0 || !is_alnum(t[i - 1]) || (t[i].is_ascii_digit() != t[i - 1].is_ascii_digit());
        if boundary {
            starts.push(i);
        }
    }
    starts
}

fn ends_word(t: &[char], end: usize) -> bool {
    end >= t.len() || !is_alnum(t[end])
}

/// A prepared query: folded text plus whitespace-separated tokens.
#[derive(Debug, Clone)]
pub struct Query {
    chars: Vec<char>,
    tokens: Vec<Vec<char>>,
}

impl Query {
    pub fn new(raw: &str) -> Self {
        let text = fold(raw.trim());
        let text = text.split_whitespace().collect::<Vec<_>>().join(" ");
        let tokens = text.split(' ').filter(|t| !t.is_empty()).map(|t| t.chars().collect()).collect();
        Self { chars: text.chars().collect(), tokens }
    }

    pub fn is_empty(&self) -> bool {
        self.chars.is_empty()
    }

    /// Cheap pre-filter: every token must appear as a subsequence of `folded`
    /// (tokens long enough for typo matching are let through when typos are allowed).
    pub fn may_match(&self, folded: &str, allow_typos: bool) -> bool {
        self.tokens.iter().all(|token| {
            if allow_typos && token.len() >= 4 {
                return true;
            }
            let mut it = folded.chars();
            token.iter().all(|qc| it.any(|tc| tc == *qc))
        })
    }

    /// Scores a candidate that has already been passed through [`fold`].
    pub fn score(&self, folded: &str, allow_typos: bool) -> Option<u32> {
        if self.is_empty() {
            return None;
        }
        let t: Vec<char> = folded.chars().collect();
        let whole = score_token(&self.chars, &t, allow_typos);
        if self.tokens.len() <= 1 {
            return whole;
        }
        let mut sum = 0u32;
        for token in &self.tokens {
            sum += score_token(token, &t, allow_typos)?;
        }
        let combined = (sum / self.tokens.len() as u32) * 9 / 10;
        Some(whole.unwrap_or(0).max(combined))
    }
}

/// Lowest score of a contiguous (substring or better) match. Secondary fields
/// such as keywords only count from this tier up, so scattered letters in a
/// long keyword list do not produce noise.
pub const CONTIGUOUS_MATCH: u32 = 540;

pub fn score_token(q: &[char], t: &[char], allow_typos: bool) -> Option<u32> {
    if q.is_empty() || t.is_empty() {
        return None;
    }
    if q == t {
        return Some(1000);
    }
    let words = word_starts(t);
    let mut best: Option<u32> = None;

    if t.len() > q.len() && t.starts_with(q) {
        if ends_word(t, q.len()) {
            bump(&mut best, 950);
        } else {
            bump(&mut best, 880 + (20 * q.len() / t.len()) as u32);
        }
    }
    for (i, &ws) in words.iter().enumerate().skip(1) {
        if t[ws..].starts_with(q) {
            let rank = i.min(10) as u32;
            if ends_word(t, ws + q.len()) {
                bump(&mut best, 920 - rank * 5);
            } else {
                bump(&mut best, 800 - rank * 8);
            }
        }
    }
    if q.len() >= 2 && words.len() >= q.len() {
        let initials: Vec<char> = words.iter().map(|&w| t[w]).collect();
        if initials.starts_with(q) {
            bump(&mut best, 760);
        } else if initials.len() <= 5 && initials[0] == q[0] && is_subsequence(q, &initials) {
            bump(&mut best, 700);
        }
    }
    if best.is_none() {
        if let Some(pos) = find(t, q) {
            bump(&mut best, 600 - pos.min(60) as u32);
        }
    }
    if best.is_none() && q.len() >= 2 {
        if let Some(s) = subsequence_score(q, t, &words) {
            bump(&mut best, s);
        }
    }
    if best.is_none() && allow_typos && q.len() >= 4 {
        if let Some(s) = typo_score(q, t, &words) {
            bump(&mut best, s);
        }
    }
    best
}

fn bump(best: &mut Option<u32>, score: u32) {
    *best = Some(best.map_or(score, |b| b.max(score)));
}

fn find(t: &[char], q: &[char]) -> Option<usize> {
    if q.len() > t.len() {
        return None;
    }
    (0..=t.len() - q.len()).find(|&i| t[i..i + q.len()] == *q)
}

fn is_subsequence(q: &[char], t: &[char]) -> bool {
    let mut it = t.iter();
    q.iter().all(|qc| it.any(|tc| tc == qc))
}

fn subsequence_score(q: &[char], t: &[char], words: &[usize]) -> Option<u32> {
    let mut positions = Vec::with_capacity(q.len());
    let mut ti = 0;
    for &qc in q {
        while ti < t.len() && t[ti] != qc {
            ti += 1;
        }
        if ti == t.len() {
            return None;
        }
        positions.push(ti);
        ti += 1;
    }
    let span = positions.last().unwrap() - positions[0] + 1;
    if span > (q.len() * 3).max(q.len() + 6) {
        return None; // scattered matches across a long name are noise
    }
    let mut score: i32 = 0;
    for (i, &p) in positions.iter().enumerate() {
        if words.binary_search(&p).is_ok() {
            score += 12;
        }
        if i > 0 {
            let gap = p - positions[i - 1] - 1;
            if gap == 0 {
                score += 10;
            } else {
                score -= gap.min(8) as i32;
            }
        }
    }
    score -= positions[0].min(10) as i32;
    let max = 22 * q.len() as i32;
    let ratio = (score.max(0) as f32 / max as f32).min(1.0);
    Some(100 + (ratio * 400.0) as u32)
}

fn typo_score(q: &[char], t: &[char], words: &[usize]) -> Option<u32> {
    for (i, &ws) in words.iter().enumerate() {
        let end = (ws..t.len()).find(|&j| !is_alnum(t[j])).unwrap_or(t.len());
        let word = &t[ws..end];
        for len in [q.len(), q.len() + 1, q.len().saturating_sub(1)] {
            if len == 0 || len > word.len() {
                continue;
            }
            if osa_distance(q, &word[..len]) <= 1 {
                return Some(300 - (i.min(4) as u32) * 5);
            }
        }
    }
    None
}

/// Optimal string alignment distance (Levenshtein + adjacent transpositions).
fn osa_distance(a: &[char], b: &[char]) -> usize {
    let (n, m) = (a.len(), b.len());
    let mut d = vec![vec![0usize; m + 1]; n + 1];
    for (i, row) in d.iter_mut().enumerate() {
        row[0] = i;
    }
    for (j, cell) in d[0].iter_mut().enumerate() {
        *cell = j;
    }
    for i in 1..=n {
        for j in 1..=m {
            let cost = usize::from(a[i - 1] != b[j - 1]);
            d[i][j] = (d[i - 1][j] + 1).min(d[i][j - 1] + 1).min(d[i - 1][j - 1] + cost);
            if i > 1 && j > 1 && a[i - 1] == b[j - 2] && a[i - 2] == b[j - 1] {
                d[i][j] = d[i][j].min(d[i - 2][j - 2] + 1);
            }
        }
    }
    d[n][m]
}

#[cfg(test)]
mod tests {
    use super::*;

    fn s(q: &str, t: &str) -> Option<u32> {
        Query::new(q).score(&fold(t), true)
    }

    #[test]
    fn folds_case_and_diacritics() {
        assert_eq!(fold("Écran Straße"), "ecran strasse");
        assert_eq!(fold("ÅNGSTRÖM"), "angstrom");
        assert_eq!(s("ecran", "Écran"), Some(1000));
    }

    #[test]
    fn tier_order_is_respected() {
        let exact = s("firefox", "Firefox").unwrap();
        let prefix = s("fire", "Firefox").unwrap();
        let word = s("code", "Visual Studio Code").unwrap();
        let word_prefix = s("stud", "Visual Studio Code").unwrap();
        let acronym = s("vsc", "Visual Studio Code").unwrap();
        let substring = s("fox", "Firefox").unwrap();
        let fuzzy = s("frfx", "Firefox").unwrap();
        assert!(exact > word && word > prefix, "{exact} {word} {prefix}");
        assert!(prefix > word_prefix && word_prefix > acronym && acronym > substring && substring > fuzzy);
    }

    #[test]
    fn whole_word_beats_prefix_of_longer_word() {
        assert!(s("code", "Visual Studio Code").unwrap() > s("code", "Codecs").unwrap());
    }

    #[test]
    fn multi_word_queries() {
        assert_eq!(s("visual studio", "Visual Studio Code"), Some(950));
        assert!(s("studio visual", "Visual Studio Code").is_some(), "tokens in any order");
        assert!(s("visual studio", "Visual Studio Code").unwrap() > s("studio visual", "Visual Studio Code").unwrap());
        assert_eq!(s("visual banana", "Visual Studio Code"), None, "every token must match");
    }

    #[test]
    fn acronyms_and_separators() {
        assert_eq!(s("gt", "gnome-terminal"), Some(760));
        assert_eq!(s("vc", "Visual Studio Code"), Some(700));
        assert_eq!(s("terminal", "gnome-terminal"), Some(920 - 5));
        assert_eq!(s("q4", "report_q4.pdf"), Some(920 - 5));
        assert!(s("lo", "LibreOffice Writer").is_some());
    }

    #[test]
    fn rejects_scattered_and_unrelated() {
        assert_eq!(s("xyz", "Firefox"), None);
        assert_eq!(s("ax", "a very long name that eventually has x"), None);
    }

    #[test]
    fn tolerates_one_typo_only_when_allowed() {
        assert_eq!(s("fierfox", "Firefox"), Some(300), "adjacent transposition");
        assert_eq!(s("termnial", "Terminal"), Some(300));
        assert_eq!(Query::new("fierfox").score(&fold("Firefox"), false), None);
        assert!(s("firfox", "Firefox").is_some(), "missing letter is still a subsequence");
    }

    #[test]
    fn prefilter_never_rejects_a_scorable_match() {
        let names = ["Firefox", "Visual Studio Code", "gnome-terminal", "report_q4.pdf", "Écran"];
        for q in ["fire", "vsc", "code", "term", "q4", "ecran", "studio visual", "frfx"] {
            let query = Query::new(q);
            for n in names {
                if query.score(&fold(n), false).is_some() {
                    assert!(query.may_match(&fold(n), false), "{q} vs {n}");
                }
            }
        }
        assert!(!Query::new("zzz").may_match("firefox", false));
    }

    #[test]
    fn contiguous_threshold_separates_substring_from_scattered() {
        assert!(s("fox", "Firefox").unwrap() >= CONTIGUOUS_MATCH);
        assert!(s("frfx", "Firefox").unwrap() < CONTIGUOUS_MATCH);
    }

    #[test]
    fn osa_distance_basics() {
        let c = |s: &str| s.chars().collect::<Vec<_>>();
        assert_eq!(osa_distance(&c("abc"), &c("abc")), 0);
        assert_eq!(osa_distance(&c("abc"), &c("acb")), 1);
        assert_eq!(osa_distance(&c("abc"), &c("ab")), 1);
        assert_eq!(osa_distance(&c("abc"), &c("xyz")), 3);
    }
}
