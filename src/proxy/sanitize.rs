//! Pure, unit-tested content sanitization for proxied messages.
//!
//! Defense-in-depth against pings: a relayed remote message must never be able to fire an
//! `@everyone`/`@here` or resolve a `<@user>` / `<@&role>` mention inside Discord. The webhook
//! execute *also* sets an empty `allowed_mentions` (the authoritative API-level guard); this text
//! transform is the second layer and additionally stops broken mention markup from rendering.

/// Zero-width space inserted after every `@` to break Discord mention/ping syntax.
const ZWSP: &str = "\u{200b}";

/// Discord's hard limit on a message's content length, in characters.
pub const MAX_MESSAGE_CHARS: usize = 2000;

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

/// Prepare remote content for a webhook relay, or `None` if there is nothing to send.
///
/// Sanitizes pings, then clamps to Discord's [`MAX_MESSAGE_CHARS`] limit — the clamp is applied
/// *after* sanitizing because the sanitizer grows the string (each `@` becomes `@` + ZWSP), so a
/// remote message just under the limit could otherwise cross it and 400 the execute. Returns
/// `None` when nothing remains to send (empty/whitespace — e.g. a media-only remote message;
/// media relay is not yet supported), so the caller can skip without a Discord error.
pub fn prepare_relay_content(input: &str) -> Option<String> {
    let prepared: String = sanitize_content(input)
        .chars()
        .take(MAX_MESSAGE_CHARS)
        .collect();
    if prepared.trim().is_empty() {
        None
    } else {
        Some(prepared)
    }
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

    // --- prepare_relay_content ---

    #[test]
    fn test_prepare_empty_is_none() {
        assert_eq!(prepare_relay_content(""), None);
    }

    #[test]
    fn test_prepare_whitespace_only_is_none() {
        assert_eq!(prepare_relay_content("   \n\t"), None);
    }

    #[test]
    fn test_prepare_normal_is_some_and_sanitized() {
        let out = prepare_relay_content("hi @everyone").expect("non-empty input yields Some");
        assert!(!out.contains("@everyone"));
    }

    #[test]
    fn test_prepare_clamps_to_max_chars() {
        let long = "a".repeat(MAX_MESSAGE_CHARS + 500);
        let out = prepare_relay_content(&long).expect("non-empty");
        assert_eq!(out.chars().count(), MAX_MESSAGE_CHARS);
    }

    #[test]
    fn test_prepare_clamps_after_sanitizer_growth() {
        // Pure '@' input: each becomes '@' + ZWSP, so the sanitized string is far longer than the
        // input. The clamp must run AFTER sanitizing, landing at exactly the limit.
        let ats = "@".repeat(MAX_MESSAGE_CHARS);
        let out = prepare_relay_content(&ats).expect("non-empty");
        assert_eq!(out.chars().count(), MAX_MESSAGE_CHARS);
    }
}
