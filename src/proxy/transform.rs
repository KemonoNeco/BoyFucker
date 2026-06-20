//! Pure, unit-tested decisions for the outbound (Discord → remote) direction.
//!
//! [`should_relay`] is the loop-prevention + noise gate; [`format_outbound`] renders a Discord
//! message for a platform that has no per-message author identity (unlike Discord webhooks).

/// Decide whether a Discord message should be relayed outbound to the remote platform.
///
/// Returns `false` for:
/// - any webhook-authored message (`webhook_id.is_some()`) — this includes our *own* proxied
///   echoes, so it is the core loop guard; we do not mirror foreign webhooks/bots either;
/// - the bot's own messages (`author_id == bot_id`);
/// - empty / whitespace-only content (nothing to relay, or unreadable without MESSAGE_CONTENT).
///
/// `bot_id` is passed in by the caller (resolved from the cache) — never looked up here, so this
/// stays pure.
pub fn should_relay(webhook_id: Option<u64>, author_id: u64, bot_id: u64, content: &str) -> bool {
    webhook_id.is_none() && author_id != bot_id && !content.trim().is_empty()
}

/// Render an outbound message body for a remote platform without per-message author display.
///
/// Discord webhooks let each remote sender appear as themselves; most other platforms (e.g. a
/// single Telegram bot) cannot, so the Discord author is prefixed into the body instead.
pub fn format_outbound(author: &str, content: &str) -> String {
    format!("{author}: {content}")
}

#[cfg(test)]
mod tests {
    use super::*;

    const BOT: u64 = 99;

    #[test]
    fn test_should_relay_normal_user_message_is_true() {
        assert!(should_relay(None, 1, BOT, "hello"));
    }

    #[test]
    fn test_should_relay_webhook_message_is_false() {
        // Our own proxied echo (and any webhook) must not be mirrored back out — the loop guard.
        assert!(!should_relay(Some(42), 1, BOT, "hello"));
    }

    #[test]
    fn test_should_relay_bot_own_message_is_false() {
        assert!(!should_relay(None, BOT, BOT, "hello"));
    }

    #[test]
    fn test_should_relay_empty_content_is_false() {
        assert!(!should_relay(None, 1, BOT, ""));
    }

    #[test]
    fn test_should_relay_whitespace_content_is_false() {
        assert!(!should_relay(None, 1, BOT, "   \n\t "));
    }

    #[test]
    fn test_should_relay_webhook_dominates_even_with_valid_content() {
        // webhook_id present must veto regardless of a non-empty body / distinct author.
        assert!(!should_relay(Some(7), 1, BOT, "real content"));
    }

    #[test]
    fn test_format_outbound_prefixes_author() {
        assert_eq!(format_outbound("Alice", "hi there"), "Alice: hi there");
    }

    #[test]
    fn test_format_outbound_preserves_content_verbatim() {
        assert_eq!(format_outbound("Bob", "line1\nline2"), "Bob: line1\nline2");
    }
}
