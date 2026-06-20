//! Pure, unit-tested derivation of a Discord-legal webhook username.
//!
//! The per-message `username` override on a webhook execute is NOT validated by serenity, so this
//! is the *sole* guard before the value reaches Discord (which 400s on a bad one). Discord's rules
//! for a webhook username override: 1–80 characters, must not contain `clyde` or `discord`
//! (case-insensitive), and must not be the reserved names `everyone` / `here`. This function is
//! infallible — anything that can't be made legal collapses to a safe fallback, so a relay never
//! fails on an odd remote display name.

/// Used when the remote name is empty, reserved, or stripped down to nothing.
const FALLBACK: &str = "user";

/// Discord's hard cap on a webhook username override.
const MAX_LEN: usize = 80;

/// Substrings Discord forbids anywhere in a webhook username (case-insensitive).
const FORBIDDEN_SUBSTRINGS: [&str; 2] = ["discord", "clyde"];

/// Remove every case-insensitive occurrence of an ASCII `needle` from `haystack`.
///
/// Matching is ASCII-case-insensitive only (the needles are ASCII), so non-ASCII characters in
/// the haystack are compared and preserved exactly — no Unicode case folding that could change
/// length and corrupt the output.
fn strip_ci(haystack: &str, needle: &str) -> String {
    let chars: Vec<char> = haystack.chars().collect();
    let nlen = needle.chars().count();
    let mut out = String::with_capacity(haystack.len());
    let mut i = 0;
    while i < chars.len() {
        if i + nlen <= chars.len() {
            let window: String = chars[i..i + nlen].iter().collect();
            if window.eq_ignore_ascii_case(needle) {
                i += nlen; // skip the matched needle entirely
                continue;
            }
        }
        out.push(chars[i]);
        i += 1;
    }
    out
}

/// Turn an arbitrary remote display name into a Discord-legal webhook username.
pub fn derive_webhook_username(raw: &str) -> String {
    // Strip to a fixed point: a single removal can splice the surrounding text into a *fresh*
    // forbidden substring (e.g. "clclydeyde" → "clyde"), so repeat until nothing changes.
    let mut name = raw.trim().to_string();
    loop {
        let before = name.clone();
        for needle in FORBIDDEN_SUBSTRINGS {
            name = strip_ci(&name, needle);
        }
        if name == before {
            break;
        }
    }
    let name = name.trim();

    // Reserved exact names, or nothing left after stripping → fallback.
    if name.is_empty() || name.eq_ignore_ascii_case("everyone") || name.eq_ignore_ascii_case("here")
    {
        return FALLBACK.to_string();
    }

    // Cap at 80 *characters* (not bytes), so a multi-byte char is never split.
    name.chars().take(MAX_LEN).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_normal_name_unchanged() {
        assert_eq!(derive_webhook_username("Alice"), "Alice");
    }

    #[test]
    fn test_outer_whitespace_trimmed() {
        assert_eq!(derive_webhook_username("  Bob  "), "Bob");
    }

    #[test]
    fn test_over_long_name_truncated_to_80_chars() {
        let long = "a".repeat(200);
        assert_eq!(derive_webhook_username(&long).chars().count(), MAX_LEN);
    }

    #[test]
    fn test_multibyte_name_not_split_at_boundary() {
        // 100 multi-byte chars: truncation must count chars, not bytes (no panic, exactly 80).
        let long = "é".repeat(100);
        assert_eq!(derive_webhook_username(&long).chars().count(), MAX_LEN);
    }

    #[test]
    fn test_pure_discord_name_falls_back() {
        // Stripping "discord" leaves nothing → fallback rather than empty username.
        assert_eq!(derive_webhook_username("discord"), FALLBACK);
    }

    #[test]
    fn test_clyde_stripped_case_insensitively() {
        let out = derive_webhook_username("ClydeBot");
        assert!(
            !out.to_lowercase().contains("clyde"),
            "clyde must be stripped regardless of case, got: {out:?}"
        );
    }

    #[test]
    fn test_embedded_discord_stripped() {
        let out = derive_webhook_username("Cool Discord User");
        assert!(
            !out.to_lowercase().contains("discord"),
            "embedded discord must be stripped, got: {out:?}"
        );
    }

    #[test]
    fn test_spliced_clyde_stripped_to_fixed_point() {
        // A single left-to-right strip leaves "clyde" here; the fixed-point loop must finish it.
        let out = derive_webhook_username("clclydeyde");
        assert!(
            !out.to_lowercase().contains("clyde"),
            "splice-formed clyde must be removed to a fixed point, got: {out:?}"
        );
    }

    #[test]
    fn test_spliced_discord_stripped_to_fixed_point() {
        let out = derive_webhook_username("disdiscordcord");
        assert!(
            !out.to_lowercase().contains("discord"),
            "splice-formed discord must be removed to a fixed point, got: {out:?}"
        );
    }

    #[test]
    fn test_reserved_everyone_falls_back() {
        assert_eq!(derive_webhook_username("everyone"), FALLBACK);
    }

    #[test]
    fn test_reserved_here_falls_back_case_insensitive() {
        assert_eq!(derive_webhook_username("HERE"), FALLBACK);
    }

    #[test]
    fn test_empty_falls_back() {
        assert_eq!(derive_webhook_username(""), FALLBACK);
    }

    #[test]
    fn test_whitespace_only_falls_back() {
        assert_eq!(derive_webhook_username("   "), FALLBACK);
    }

    #[test]
    fn test_unicode_name_preserved() {
        // A short non-ASCII name with no forbidden substrings must pass through intact.
        assert_eq!(derive_webhook_username("Jürgen"), "Jürgen");
    }
}
