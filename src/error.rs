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

#[derive(Debug, Error, PartialEq, Eq)]
pub enum PollError {
    #[error("Poll question must not be empty")]
    EmptyQuestion,

    #[error("Poll question must be at most 300 characters")]
    QuestionTooLong,

    #[error("A poll needs at least 2 options (separate them with |)")]
    TooFewOptions,

    #[error("A poll can have at most 10 options")]
    TooManyOptions,

    #[error("Each poll option must be at most 55 characters")]
    OptionTooLong,

    #[error("Invalid poll duration: use a format like 6h or 2d (1 hour to 32 days)")]
    InvalidDuration,
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

    #[test]
    fn test_poll_error_empty_question_display_non_empty() {
        let rendered = PollError::EmptyQuestion.to_string();
        assert!(
            !rendered.is_empty(),
            "expected EmptyQuestion Display to be non-empty"
        );
    }

    #[test]
    fn test_poll_error_question_too_long_display_mentions_limit() {
        let rendered = PollError::QuestionTooLong.to_string();
        assert!(
            rendered.contains("300"),
            "expected QuestionTooLong Display to mention the 300-char limit, got: {rendered}"
        );
    }

    #[test]
    fn test_poll_error_too_few_options_display_mentions_separator() {
        let rendered = PollError::TooFewOptions.to_string();
        // The message must teach the `|` separator — users reflexively type commas otherwise.
        assert!(
            rendered.contains('|'),
            "expected TooFewOptions Display to mention the | separator, got: {rendered}"
        );
    }

    #[test]
    fn test_poll_error_too_many_options_display_mentions_limit() {
        let rendered = PollError::TooManyOptions.to_string();
        assert!(
            rendered.contains("10"),
            "expected TooManyOptions Display to mention the 10-option limit, got: {rendered}"
        );
    }

    #[test]
    fn test_poll_error_option_too_long_display_mentions_limit() {
        let rendered = PollError::OptionTooLong.to_string();
        assert!(
            rendered.contains("55"),
            "expected OptionTooLong Display to mention the 55-char limit, got: {rendered}"
        );
    }

    #[test]
    fn test_poll_error_invalid_duration_display_non_empty() {
        let rendered = PollError::InvalidDuration.to_string();
        assert!(
            !rendered.is_empty(),
            "expected InvalidDuration Display to be non-empty, got: {rendered:?}"
        );
    }
}
