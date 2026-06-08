use crate::error::BotError;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Config {
    pub token: String,
}

impl Config {
    pub fn from_token(token: Option<String>) -> Result<Config, BotError> {
        let token = token.ok_or(BotError::MissingToken)?;
        if token.trim().is_empty() {
            return Err(BotError::EmptyToken);
        }
        Ok(Config { token })
    }
}

/// Reads `DISCORD_TOKEN` from the process environment and validates it via [`Config::from_token`].
///
/// Thin I/O wrapper with no test seam: a missing/non-unicode var maps to `None` (→ `MissingToken`),
/// a present value to `Some(_)`. The validation logic is unit-tested on the pure `from_token`; this
/// reader is exercised by the live run.
pub fn from_env() -> Result<Config, BotError> {
    Config::from_token(std::env::var("DISCORD_TOKEN").ok())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_from_token_none_returns_missing_token() {
        assert_eq!(Config::from_token(None), Err(BotError::MissingToken));
    }

    #[test]
    fn test_from_token_empty_string_returns_empty_token() {
        assert_eq!(
            Config::from_token(Some(String::new())),
            Err(BotError::EmptyToken)
        );
    }

    #[test]
    fn test_from_token_ascii_spaces_only_returns_empty_token() {
        assert_eq!(
            Config::from_token(Some("   ".to_string())),
            Err(BotError::EmptyToken)
        );
    }

    #[test]
    fn test_from_token_mixed_whitespace_only_returns_empty_token() {
        assert_eq!(
            Config::from_token(Some("\t\n\r".to_string())),
            Err(BotError::EmptyToken)
        );
    }

    #[test]
    fn test_from_token_typical_value_returns_ok_config() {
        let config = Config::from_token(Some("abc.def.ghi".to_string()))
            .expect("typical token should produce Ok");
        assert_eq!(config.token, "abc.def.ghi");
    }

    #[test]
    fn test_from_token_value_with_surrounding_whitespace_stored_verbatim() {
        let config = Config::from_token(Some(" a ".to_string()))
            .expect("non-whitespace token should produce Ok");
        assert_eq!(config.token, " a ");
        assert_eq!(config.token.len(), 3);
    }

    #[test]
    fn test_from_token_error_display_mentions_discord_token() {
        for token in [None, Some(String::new()), Some(" \t\n".to_string())] {
            let rendered = Config::from_token(token.clone())
                .expect_err("blank/missing token should produce Err")
                .to_string();
            assert!(
                rendered.contains("DISCORD_TOKEN"),
                "expected error Display for input {token:?} to mention DISCORD_TOKEN, got: {rendered}"
            );
        }
    }

    #[test]
    fn test_from_token_none_error_display_mentions_discord_token() {
        let rendered = Config::from_token(None)
            .expect_err("None token should produce Err")
            .to_string();
        assert!(
            rendered.contains("DISCORD_TOKEN"),
            "expected None-token error Display to mention DISCORD_TOKEN, got: {rendered}"
        );
    }

    #[test]
    fn test_from_token_empty_string_error_display_mentions_discord_token() {
        let rendered = Config::from_token(Some(String::new()))
            .expect_err("empty token should produce Err")
            .to_string();
        assert!(
            rendered.contains("DISCORD_TOKEN"),
            "expected empty-token error Display to mention DISCORD_TOKEN, got: {rendered}"
        );
    }

    #[test]
    fn test_from_token_whitespace_only_error_display_mentions_discord_token() {
        for token in ["   ".to_string(), "\t\n\r".to_string()] {
            let rendered = Config::from_token(Some(token.clone()))
                .expect_err("whitespace-only token should produce Err")
                .to_string();
            assert!(
                rendered.contains("DISCORD_TOKEN"),
                "expected whitespace-token error Display for {token:?} to mention DISCORD_TOKEN, got: {rendered}"
            );
        }
    }
}
