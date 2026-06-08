use thiserror::Error;

// STUB (tdd red phase): messages are intentionally neutral placeholders.
// The implementation-author replaces these #[error(...)] strings in green
// to mention DISCORD_TOKEN. dead_code is allowed until real call sites exist.
#[allow(dead_code)]
#[derive(Debug, Error, PartialEq, Eq)]
pub enum BotError {
    // WORK_UNIT_ID: wu-boterror-missingtoken-display
    #[error("DISCORD_TOKEN environment variable is not set")]
    MissingToken,

    // WORK_UNIT_ID: wu-boterror-emptytoken-display
    #[error("DISCORD_TOKEN environment variable is set but empty")]
    EmptyToken,
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
}
