use thiserror::Error;

#[derive(Debug, Error, PartialEq, Eq)]
pub enum BotError {
    // WORK_UNIT_ID: wu-boterror-missingtoken-display
    #[error("DISCORD_TOKEN environment variable is not set")]
    MissingToken,

    // WORK_UNIT_ID: wu-boterror-emptytoken-display
    #[error("DISCORD_TOKEN environment variable is set but empty")]
    EmptyToken,
}

// `#[allow(dead_code)]`: these variants are not yet constructed from
// non-test code. The implementation that wires ModError into the moderation
// flow lands separately; until then a non-test build would otherwise trip the
// `dead_code` lint under `-D warnings`, so the allow stays for now.
#[allow(dead_code)]
#[derive(Debug, Error, PartialEq, Eq)]
pub enum ModError {
    // WORK_UNIT_ID: wu-moderror-invalidpurgecount-display
    #[error("Purge count must be between 1 and 100 messages")]
    InvalidPurgeCount,

    // WORK_UNIT_ID: wu-moderror-invalidduration-display
    #[error("Invalid duration: use a format like 10m, 2h, or 7d")]
    InvalidDuration,

    // WORK_UNIT_ID: wu-moderror-durationtoolong-display
    #[error("Timeout duration is too long: the maximum is 28 days")]
    DurationTooLong,

    // WORK_UNIT_ID: wu-moderror-cannottargetself-display
    #[error("You cannot target yourself with this action")]
    CannotTargetSelf,

    // WORK_UNIT_ID: wu-moderror-cannottargetbot-display
    #[error("You cannot target the bot with this action")]
    CannotTargetBot,

    // WORK_UNIT_ID: wu-moderror-cannottargetowner-display
    #[error("You cannot moderate the guild owner")]
    CannotTargetOwner,

    // WORK_UNIT_ID: wu-moderror-insufficienthierarchy-display
    #[error("Your highest role is not high enough in the hierarchy to moderate this member")]
    InsufficientHierarchy,

    // WORK_UNIT_ID: wu-moderror-invalidbandeletedays-display
    #[error("Ban message-deletion days must be between 0 and 7")]
    InvalidBanDeleteDays,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_missing_token_display_mentions_discord_token() {
        let rendered = BotError::MissingToken.to_string();
        assert!(
            rendered.contains("DISCORD_TOKEN"),
            "expected MissingToken Display to mention DISCORD_TOKEN, got: {rendered}"
        );
    }

    #[test]
    fn test_empty_token_display_mentions_discord_token() {
        let rendered = BotError::EmptyToken.to_string();
        assert!(
            rendered.contains("DISCORD_TOKEN"),
            "expected EmptyToken Display to mention DISCORD_TOKEN, got: {rendered}"
        );
    }

    #[test]
    fn test_mod_error_invalid_purge_count_display_mentions_limit() {
        let rendered = ModError::InvalidPurgeCount.to_string();
        assert!(
            !rendered.is_empty(),
            "expected InvalidPurgeCount Display to be non-empty"
        );
        assert!(
            rendered.contains("100"),
            "expected InvalidPurgeCount Display to mention the limit 100, got: {rendered}"
        );
    }

    #[test]
    fn test_mod_error_invalid_duration_display_non_empty() {
        let rendered = ModError::InvalidDuration.to_string();
        assert!(
            !rendered.is_empty(),
            "expected InvalidDuration Display to be non-empty, got: {rendered:?}"
        );
    }

    #[test]
    fn test_mod_error_duration_too_long_display_mentions_limit() {
        let rendered = ModError::DurationTooLong.to_string();
        assert!(
            !rendered.is_empty(),
            "expected DurationTooLong Display to be non-empty"
        );
        assert!(
            rendered.contains("28"),
            "expected DurationTooLong Display to mention the 28-day cap, got: {rendered}"
        );
    }

    #[test]
    fn test_mod_error_cannot_target_self_display_mentions_self() {
        let rendered = ModError::CannotTargetSelf.to_string();
        assert!(
            !rendered.is_empty(),
            "expected CannotTargetSelf Display to be non-empty"
        );
        assert!(
            rendered.to_lowercase().contains("self"),
            "expected CannotTargetSelf Display to mention 'self', got: {rendered}"
        );
    }

    #[test]
    fn test_mod_error_cannot_target_bot_display_mentions_bot() {
        let rendered = ModError::CannotTargetBot.to_string();
        assert!(
            !rendered.is_empty(),
            "expected CannotTargetBot Display to be non-empty"
        );
        assert!(
            rendered.to_lowercase().contains("bot"),
            "expected CannotTargetBot Display to mention 'bot', got: {rendered}"
        );
    }

    #[test]
    fn test_mod_error_cannot_target_owner_display_mentions_owner() {
        let rendered = ModError::CannotTargetOwner.to_string();
        assert!(
            !rendered.is_empty(),
            "expected CannotTargetOwner Display to be non-empty"
        );
        assert!(
            rendered.to_lowercase().contains("owner"),
            "expected CannotTargetOwner Display to mention 'owner', got: {rendered}"
        );
    }

    #[test]
    fn test_mod_error_insufficient_hierarchy_display_mentions_role_or_hierarchy() {
        let rendered = ModError::InsufficientHierarchy.to_string();
        assert!(
            !rendered.is_empty(),
            "expected InsufficientHierarchy Display to be non-empty"
        );
        let lower = rendered.to_lowercase();
        assert!(
            lower.contains("hierarchy") || lower.contains("role"),
            "expected InsufficientHierarchy Display to mention 'hierarchy' or 'role', got: {rendered}"
        );
    }

    #[test]
    fn test_mod_error_invalid_ban_delete_days_display_mentions_limit() {
        let rendered = ModError::InvalidBanDeleteDays.to_string();
        assert!(
            !rendered.is_empty(),
            "expected InvalidBanDeleteDays Display to be non-empty"
        );
        assert!(
            rendered.contains("7"),
            "expected InvalidBanDeleteDays Display to mention the limit 7, got: {rendered}"
        );
    }
}
