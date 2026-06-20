//! Pure, unit-tested content sanitization for proxied messages.
//!
//! Defense-in-depth against pings: a relayed remote message must never be able to fire an
//! `@everyone`/`@here` or resolve a `<@user>` / `<@&role>` mention inside Discord. The webhook
//! execute *also* sets an empty `allowed_mentions` (the authoritative API-level guard); this text
//! transform is the second layer and additionally stops broken mention markup from rendering.

/// Zero-width space inserted after every `@` to break Discord mention/ping syntax.
const ZWSP: &str = "\u{200b}";

/// Neutralize every ping vector in `input` by inserting a zero-width space after each `@`.
///
/// Every Discord ping form contains an `@` (`@everyone`, `@here`, `<@id>`, `<@!id>`, `<@&id>`),
/// so a single uniform rule covers them all: the `@` survives visually but is no longer adjacent
/// to the token that would make it a live mention. Text without `@` is returned unchanged.
///
/// Note: this also breaks incidental `@` usage (e.g. `a@b.com`) — acceptable for a chat relay,
/// where the safety guarantee outweighs preserving literal at-signs.
pub fn sanitize_content(input: &str) -> String {
    input.replace('@', &format!("@{ZWSP}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sanitize_neutralizes_everyone() {
        let out = sanitize_content("hey @everyone look");
        assert!(
            !out.contains("@everyone"),
            "the contiguous @everyone trigger must be broken, got: {out:?}"
        );
        assert!(out.contains(&format!("@{ZWSP}everyone")));
    }

    #[test]
    fn test_sanitize_neutralizes_here() {
        let out = sanitize_content("@here");
        assert!(!out.contains("@here"));
    }

    #[test]
    fn test_sanitize_breaks_user_mention() {
        let out = sanitize_content("ping <@123456789>");
        assert!(
            !out.contains("<@123456789>"),
            "user mention markup must be broken, got: {out:?}"
        );
    }

    #[test]
    fn test_sanitize_breaks_role_mention() {
        let out = sanitize_content("<@&987654321> heads up");
        assert!(
            !out.contains("<@&987654321>"),
            "role mention markup must be broken, got: {out:?}"
        );
    }

    #[test]
    fn test_sanitize_breaks_nickname_mention() {
        let out = sanitize_content("<@!555>");
        assert!(!out.contains("<@!555>"));
    }

    #[test]
    fn test_sanitize_leaves_benign_text_untouched() {
        // No '@' anywhere: the string must be returned byte-for-byte.
        let input = "just a normal message, no pings here";
        assert_eq!(sanitize_content(input), input);
    }

    #[test]
    fn test_sanitize_empty_is_empty() {
        assert_eq!(sanitize_content(""), "");
    }

    #[test]
    fn test_sanitize_every_at_is_followed_by_zwsp() {
        // Two pings in one message: both must be broken (the replace is global, not first-only).
        let out = sanitize_content("@everyone and @here");
        assert!(!out.contains("@everyone"));
        assert!(!out.contains("@here"));
    }
}
