//! Token estimates for exported chat text, which carries no counts.
//!
//! One documented ratio serves every provider (ADR-040): about four characters per
//! token for ASCII text, OpenAI's and Anthropic's published rule of thumb, and about
//! one and a half characters per token for other scripts, where tokenizers split text
//! more finely. Events built from these estimates are marked Estimated.

/// Estimated tokens in `text`.
#[must_use]
pub fn tokens(text: &str) -> u64 {
    let (ascii, other) = text.chars().fold((0_u64, 0_u64), |(a, o), c| {
        if c.is_ascii() { (a + 1, o) } else { (a, o + 1) }
    });
    // ascii / 4 + other / 1.5, in twelfths, rounded up.
    (ascii * 3 + other * 8).div_ceil(12)
}

#[cfg(test)]
mod tests {
    use super::tokens;

    #[test]
    fn four_ascii_characters_make_a_token() {
        assert_eq!(tokens(""), 0);
        assert_eq!(tokens("abcd"), 1);
        assert_eq!(tokens("abcde"), 2);
        assert_eq!(tokens(&"word ".repeat(80)), 100);
    }

    #[test]
    fn other_scripts_count_more_finely() {
        assert_eq!(tokens("日本語です"), 4);
        assert_eq!(tokens("naïve"), 2);
    }
}
