//! Text normalization helpers shared across the core layer.
//!
//! macOS surfaces decomposed Hangul (NFD) from a few places — filesystem
//! APIs, and some commit metadata that was originally typed there. Many
//! UI fonts and shapers draw isolated Jamo poorly, so we recompose the
//! standard L+V[+T] sequences into precomposed AC00-block syllables
//! before they reach the renderer. Non-Hangul characters pass through.

const S_BASE: u32 = 0xAC00;
const L_BASE: u32 = 0x1100;
const V_BASE: u32 = 0x1161;
const T_BASE: u32 = 0x11A7;
const L_COUNT: u32 = 19;
const V_COUNT: u32 = 21;
const T_COUNT: u32 = 28;
const N_COUNT: u32 = V_COUNT * T_COUNT;
const S_COUNT: u32 = L_COUNT * N_COUNT;

const COMPAT_CONSONANTS: [char; 19] = [
    'ㄱ', 'ㄲ', 'ㄴ', 'ㄷ', 'ㄸ', 'ㄹ', 'ㅁ', 'ㅂ', 'ㅃ', 'ㅅ', 'ㅆ', 'ㅇ', 'ㅈ', 'ㅉ', 'ㅊ', 'ㅋ',
    'ㅌ', 'ㅍ', 'ㅎ',
];

const COMPAT_VOWELS: [char; 21] = [
    'ㅏ', 'ㅐ', 'ㅑ', 'ㅒ', 'ㅓ', 'ㅔ', 'ㅕ', 'ㅖ', 'ㅗ', 'ㅘ', 'ㅙ', 'ㅚ', 'ㅛ', 'ㅜ', 'ㅝ', 'ㅞ',
    'ㅟ', 'ㅠ', 'ㅡ', 'ㅢ', 'ㅣ',
];

const COMPAT_TRAILING_CONSONANTS: [char; 27] = [
    'ㄱ', 'ㄲ', 'ㄳ', 'ㄴ', 'ㄵ', 'ㄶ', 'ㄷ', 'ㄹ', 'ㄺ', 'ㄻ', 'ㄼ', 'ㄽ', 'ㄾ', 'ㄿ', 'ㅀ', 'ㅁ',
    'ㅂ', 'ㅄ', 'ㅅ', 'ㅆ', 'ㅇ', 'ㅈ', 'ㅊ', 'ㅋ', 'ㅌ', 'ㅍ', 'ㅎ',
];

/// Recompose decomposed Hangul Jamo into precomposed syllables.
///
/// Handles both `L V` → LV-syllable and `LV T` → LVT-syllable, including
/// the mixed case where the source already had an LV syllable followed
/// by a trailing T Jamo. Everything else is preserved verbatim.
pub fn compose_hangul(input: &str) -> String {
    if !input.chars().any(needs_composition_hint) {
        return input.to_string();
    }

    let mut output = String::with_capacity(input.len());
    let chars = input.chars().collect::<Vec<_>>();
    let mut idx = 0;
    while idx < chars.len() {
        let ch = chars[idx];

        if let (Some(l_index), Some(&vowel)) = (leading_index(ch), chars.get(idx + 1)) {
            if let Some(mut v_index) = vowel_index(vowel) {
                let mut consumed = 2;
                if let Some(&next_vowel) = chars.get(idx + consumed) {
                    if let Some(combined) = compose_vowel_pair(v_index, vowel_index(next_vowel)) {
                        v_index = combined;
                        consumed += 1;
                    }
                }
                let mut t_index = 0;
                if let Some(trailing) = trailing_at(&chars, idx + consumed) {
                    t_index = trailing.index;
                    consumed += trailing.consumed;
                }
                output.push(make_syllable(l_index, v_index, t_index));
                idx += consumed;
                continue;
            }
        }

        if let Some(&next) = chars.get(idx + 1) {
            if let Some(syllable) = compose_lvt(ch, next) {
                output.push(syllable);
                idx += 2;
                continue;
            }
        }

        output.push(ch);
        idx += 1;
    }
    output
}

fn needs_composition_hint(ch: char) -> bool {
    let cp = ch as u32;
    (L_BASE..L_BASE + L_COUNT).contains(&cp)
        || (V_BASE..V_BASE + V_COUNT).contains(&cp)
        || (T_BASE + 1..T_BASE + T_COUNT).contains(&cp)
        || is_compatibility_jamo(ch)
}

fn compose_lvt(lv: char, t: char) -> Option<char> {
    let lv_cp = lv as u32;
    if !(S_BASE..S_BASE + S_COUNT).contains(&lv_cp) {
        return None;
    }
    if !(lv_cp - S_BASE).is_multiple_of(T_COUNT) {
        return None;
    }
    let t_index = trailing_index(t)?;
    char::from_u32(lv_cp + t_index)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Trailing {
    index: u32,
    consumed: usize,
}

fn trailing_at(chars: &[char], idx: usize) -> Option<Trailing> {
    let first = *chars.get(idx)?;
    let first_index = trailing_index(first)?;

    if let Some(&second) = chars.get(idx + 1) {
        if vowel_index(second).is_some() {
            return None;
        }
        if let Some(cluster_index) = trailing_cluster_index(first, second) {
            if chars
                .get(idx + 2)
                .is_none_or(|third| vowel_index(*third).is_none())
            {
                return Some(Trailing {
                    index: cluster_index,
                    consumed: 2,
                });
            }
        }
    }

    Some(Trailing {
        index: first_index,
        consumed: 1,
    })
}

fn make_syllable(l_index: u32, v_index: u32, t_index: u32) -> char {
    char::from_u32(S_BASE + l_index * N_COUNT + v_index * T_COUNT + t_index).unwrap_or('\u{FFFD}')
}

fn leading_index(ch: char) -> Option<u32> {
    let cp = ch as u32;
    if (L_BASE..L_BASE + L_COUNT).contains(&cp) {
        return Some(cp - L_BASE);
    }
    COMPAT_CONSONANTS
        .iter()
        .position(|candidate| *candidate == ch)
        .map(|index| index as u32)
}

fn vowel_index(ch: char) -> Option<u32> {
    let cp = ch as u32;
    if (V_BASE..V_BASE + V_COUNT).contains(&cp) {
        return Some(cp - V_BASE);
    }
    COMPAT_VOWELS
        .iter()
        .position(|candidate| *candidate == ch)
        .map(|index| index as u32)
}

fn trailing_index(ch: char) -> Option<u32> {
    let cp = ch as u32;
    if (T_BASE + 1..T_BASE + T_COUNT).contains(&cp) {
        return Some(cp - T_BASE);
    }
    COMPAT_TRAILING_CONSONANTS
        .iter()
        .position(|candidate| *candidate == ch)
        .map(|index| index as u32 + 1)
}

fn compose_vowel_pair(first: u32, second: Option<u32>) -> Option<u32> {
    match (first, second?) {
        (8, 0) => Some(9),    // ㅗ + ㅏ = ㅘ
        (8, 1) => Some(10),   // ㅗ + ㅐ = ㅙ
        (8, 20) => Some(11),  // ㅗ + ㅣ = ㅚ
        (13, 4) => Some(14),  // ㅜ + ㅓ = ㅝ
        (13, 5) => Some(15),  // ㅜ + ㅔ = ㅞ
        (13, 20) => Some(16), // ㅜ + ㅣ = ㅟ
        (18, 20) => Some(19), // ㅡ + ㅣ = ㅢ
        _ => None,
    }
}

fn trailing_cluster_index(first: char, second: char) -> Option<u32> {
    match (first, second) {
        ('ㄱ', 'ㅅ') => Some(3),
        ('ㄴ', 'ㅈ') => Some(5),
        ('ㄴ', 'ㅎ') => Some(6),
        ('ㄹ', 'ㄱ') => Some(9),
        ('ㄹ', 'ㅁ') => Some(10),
        ('ㄹ', 'ㅂ') => Some(11),
        ('ㄹ', 'ㅅ') => Some(12),
        ('ㄹ', 'ㅌ') => Some(13),
        ('ㄹ', 'ㅍ') => Some(14),
        ('ㄹ', 'ㅎ') => Some(15),
        ('ㅂ', 'ㅅ') => Some(18),
        _ => None,
    }
}

pub fn is_hangul_compatibility_jamo(ch: char) -> bool {
    is_compatibility_jamo(ch)
}

fn is_compatibility_jamo(ch: char) -> bool {
    COMPAT_CONSONANTS.contains(&ch)
        || COMPAT_VOWELS.contains(&ch)
        || COMPAT_TRAILING_CONSONANTS.contains(&ch)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn composes_simple_lv_syllable() {
        // U+1100 + U+1161 => U+AC00.
        let input = "\u{1100}\u{1161}";
        assert_eq!(compose_hangul(input), "\u{AC00}");
    }

    #[test]
    fn composes_lvt_syllable() {
        // U+1100 + U+1161 + U+11AB => U+AC04.
        let input = "\u{1100}\u{1161}\u{11AB}";
        assert_eq!(compose_hangul(input), "\u{AC04}");
    }

    #[test]
    fn composes_mixed_korean_text() {
        // Korean text in fully decomposed NFD form (L V [T] for each syllable).
        let input = "\u{110E}\u{116C}\u{1109}\u{1165}\u{11BC}\u{110C}\u{1175}\u{11AB}";
        assert_eq!(compose_hangul(input), "\u{CD5C}\u{C131}\u{C9C4}");
    }

    #[test]
    fn passes_through_ascii() {
        assert_eq!(compose_hangul("hello world"), "hello world");
    }

    #[test]
    fn passes_through_already_composed_hangul() {
        let input = "\u{C548}\u{B155}\u{D558}\u{C138}\u{C694}";
        assert_eq!(compose_hangul(input), input);
    }

    #[test]
    fn handles_mixed_decomposed_and_ascii() {
        // "hello " followed by U+AC00 in decomposed form.
        let input = "hello \u{1100}\u{1161}";
        assert_eq!(compose_hangul(input), "hello \u{AC00}");
    }

    #[test]
    fn composes_syllable_plus_trailing_t() {
        // Precomposed U+AC00 followed by U+11AB => U+AC04.
        let input = "\u{AC00}\u{11AB}";
        assert_eq!(compose_hangul(input), "\u{AC04}");
    }

    #[test]
    fn composes_compatibility_jamo_syllables() {
        assert_eq!(compose_hangul("ㅎㅏㅇㅣ"), "하이");
    }

    #[test]
    fn composes_compatibility_jamo_with_final_consonants() {
        assert_eq!(compose_hangul("ㅎㅏㄴㄱㅡㄹ"), "한글");
    }

    #[test]
    fn leaves_compatibility_consonant_as_next_initial_before_vowel() {
        assert_eq!(compose_hangul("ㅎㅏㄴㅏ"), "하나");
    }

    #[test]
    fn composes_compatibility_vowel_pairs() {
        assert_eq!(compose_hangul("ㅇㅘ"), "와");
    }

    #[test]
    fn composes_compatibility_trailing_clusters() {
        assert_eq!(compose_hangul("ㄱㅏㅂㅅ"), "값");
    }
}
