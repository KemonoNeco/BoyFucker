use thiserror::Error;

#[derive(Debug, Error, PartialEq, Eq)]
pub enum BotError {
    #[error("no token was provided")]
    MissingToken,
    #[error("the provided token was empty")]
    EmptyToken,
}

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
}
