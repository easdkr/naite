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
    let mut chars = input.chars().peekable();
    while let Some(ch) = chars.next() {
        let mut current = ch;

        if let Some(&next) = chars.peek() {
            if let Some(syllable) = compose_lv(current, next) {
                chars.next();
                current = syllable;
            }
        }

        if let Some(&next) = chars.peek() {
            if let Some(syllable) = compose_lvt(current, next) {
                chars.next();
                current = syllable;
            }
        }

        output.push(current);
    }
    output
}

fn needs_composition_hint(ch: char) -> bool {
    let cp = ch as u32;
    (L_BASE..L_BASE + L_COUNT).contains(&cp)
        || (V_BASE..V_BASE + V_COUNT).contains(&cp)
        || (T_BASE + 1..T_BASE + T_COUNT).contains(&cp)
}

fn compose_lv(l: char, v: char) -> Option<char> {
    let l_cp = l as u32;
    let v_cp = v as u32;
    if !(L_BASE..L_BASE + L_COUNT).contains(&l_cp) {
        return None;
    }
    if !(V_BASE..V_BASE + V_COUNT).contains(&v_cp) {
        return None;
    }
    let l_index = l_cp - L_BASE;
    let v_index = v_cp - V_BASE;
    let lv_index = l_index * N_COUNT + v_index * T_COUNT;
    char::from_u32(S_BASE + lv_index)
}

fn compose_lvt(lv: char, t: char) -> Option<char> {
    let lv_cp = lv as u32;
    let t_cp = t as u32;
    if !(S_BASE..S_BASE + S_COUNT).contains(&lv_cp) {
        return None;
    }
    if !(lv_cp - S_BASE).is_multiple_of(T_COUNT) {
        return None;
    }
    if !(T_BASE + 1..T_BASE + T_COUNT).contains(&t_cp) {
        return None;
    }
    let t_index = t_cp - T_BASE;
    char::from_u32(lv_cp + t_index)
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
}
