//! `/poll` command: a pure, unit-tested input layer over Discord's native poll feature.
//!
//! The validators here (question / options / duration) are pure — they take their input as
//! arguments and return `Result<_, PollError>` — so they can be unit-tested in isolation. The
//! `poll` handler is thin glue that runs them and sends a native [`serenity::CreatePoll`] as the
//! command's reply (Discord then tallies the votes itself).

use std::time::Duration;

// Poll validators return the canonical PollError from `src/error.rs`; the Display strings there
// are the user-facing contract these helpers feed into.
use crate::error::PollError;

/// Discord's native-poll limits (measured in characters, i.e. codepoints — not bytes).
const MAX_QUESTION_CHARS: usize = 300;
const MAX_OPTION_CHARS: usize = 55;
const MIN_OPTIONS: usize = 2;
const MAX_OPTIONS: usize = 10;

/// The separator users type between poll options (echoed in the option description and the
/// too-few-options error so a comma-typer is redirected).
pub const OPTION_SEPARATOR: char = '|';

/// Discord's poll-duration window in seconds: 1 hour to 32 days. Discord rounds the duration
/// down to whole hours, so the 1-hour floor is what stops a sub-hour value from becoming 0h.
const MIN_DURATION_SECS: u64 = 3_600;
const MAX_DURATION_SECS: u64 = 32 * 86_400; // 2_764_800
/// Applied when the duration argument is omitted.
const DEFAULT_DURATION_SECS: u64 = 24 * 3_600;

/// Validate and normalize a poll question, returning the trimmed text.
///
/// Length is counted in characters to match Discord's 300-codepoint cap (a byte count would
/// falsely reject a question of valid length that contains emoji or accented characters).
pub fn validate_poll_question(input: &str) -> Result<String, PollError> {
    let trimmed = input.trim();
    if trimmed.is_empty() {
        return Err(PollError::EmptyQuestion);
    }
    if trimmed.chars().count() > MAX_QUESTION_CHARS {
        return Err(PollError::QuestionTooLong);
    }
    Ok(trimmed.to_string())
}

/// Parse the `|`-separated options string into a validated list of answer texts.
///
/// Each option is trimmed and blank entries are dropped (so trailing or doubled separators are
/// tolerated). The result must hold 2..=10 options, each at most 55 characters.
pub fn parse_poll_options(input: &str) -> Result<Vec<String>, PollError> {
    let options: Vec<String> = input
        .split(OPTION_SEPARATOR)
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(str::to_string)
        .collect();

    if options.len() < MIN_OPTIONS {
        return Err(PollError::TooFewOptions);
    }
    if options.len() > MAX_OPTIONS {
        return Err(PollError::TooManyOptions);
    }
    // Per-option length is counted in characters, matching Discord's 55-codepoint cap.
    if options.iter().any(|o| o.chars().count() > MAX_OPTION_CHARS) {
        return Err(PollError::OptionTooLong);
    }
    Ok(options)
}

/// Parse an optional duration token (`6h`, `2d`, …) into a [`Duration`] within Discord's poll
/// window. `None` yields the 24-hour default.
///
/// The grammar matches the timeout parser (a positive integer with an `s`/`m`/`h`/`d` suffix),
/// but the accepted range is the poll window: 1 hour to 32 days. Discord runs polls in whole
/// hours, rounding toward zero, so the result is snapped to whole hours here too — the returned
/// `Duration` therefore equals the poll's real runtime (`90m` → 1h, not 90m). After snapping,
/// the range is enforced on the result rather than trusting serenity's builder, which only clamps
/// on u16 overflow (a value like `1000h` would otherwise be sent as-is and rejected by Discord);
/// the 1-hour floor also rejects any sub-hour value, which would snap to zero.
pub fn parse_poll_duration(input: Option<&str>) -> Result<Duration, PollError> {
    let Some(raw) = input else {
        return Ok(Duration::from_secs(DEFAULT_DURATION_SECS));
    };
    let trimmed = raw.trim();

    // The unit suffix is the final character; split it off the magnitude.
    let (magnitude, unit) = match trimmed.char_indices().next_back() {
        Some((idx, unit_char)) => (&trimmed[..idx], unit_char),
        None => return Err(PollError::InvalidDuration),
    };
    let seconds_per_unit: u64 = match unit {
        's' => 1,
        'm' => 60,
        'h' => 3_600,
        'd' => 86_400,
        _ => return Err(PollError::InvalidDuration),
    };
    // Parsing as u64 rejects negatives, decimals, non-numeric text, a bare missing magnitude,
    // and over-wide literals (which surface as InvalidDuration, never a panic).
    let magnitude: u64 = magnitude.parse().map_err(|_| PollError::InvalidDuration)?;
    let total_secs = magnitude
        .checked_mul(seconds_per_unit)
        .ok_or(PollError::InvalidDuration)?;

    // Snap toward zero to whole hours, matching how Discord actually runs the poll, so the
    // returned Duration is truthful (a non-whole-hour value like 90m becomes exactly 1h).
    let whole_hours_secs = total_secs / 3_600 * 3_600;

    // One range check covers the sub-hour floor (anything under 1h snaps to 0) and the 32-day ceiling.
    if !(MIN_DURATION_SECS..=MAX_DURATION_SECS).contains(&whole_hours_secs) {
        return Err(PollError::InvalidDuration);
    }
    Ok(Duration::from_secs(whole_hours_secs))
}

// ---------------------------------------------------------------------------
// Command handler (I/O glue — verified by compile + live run, not unit tests).
// Validates the three inputs via the pure helpers above, then sends a native poll.
// ---------------------------------------------------------------------------

use crate::{Context, Data, Error};
use poise::serenity_prelude as serenity;

/// Reply to the invoker with a validation error, visible only to them (the channel is reserved
/// for the poll itself on the success path).
async fn reply_err(ctx: Context<'_>, err: PollError) -> Result<(), Error> {
    ctx.send(
        poise::CreateReply::default()
            .content(err.to_string())
            .ephemeral(true),
    )
    .await?;
    Ok(())
}

/// Create a native Discord poll. Discord runs the vote and tallies results itself.
#[poise::command(slash_command)]
pub async fn poll(
    ctx: Context<'_>,
    #[description = "The poll question"] question: String,
    #[description = "Options separated by | (2-10), e.g. Pizza | Sushi | Tacos"] options: String,
    #[description = "How long it runs: 6h, 2d, … (1 hour to 32 days; default 24h)"]
    duration: Option<String>,
    #[description = "Let voters pick more than one option"] multiple: Option<bool>,
) -> Result<(), Error> {
    let question = match validate_poll_question(&question) {
        Ok(q) => q,
        Err(e) => return reply_err(ctx, e).await,
    };
    let options = match parse_poll_options(&options) {
        Ok(o) => o,
        Err(e) => return reply_err(ctx, e).await,
    };
    let duration = match parse_poll_duration(duration.as_deref()) {
        Ok(d) => d,
        Err(e) => return reply_err(ctx, e).await,
    };

    let answers: Vec<serenity::CreatePollAnswer> = options
        .into_iter()
        .map(|text| serenity::CreatePollAnswer::new().text(text))
        .collect();
    // The typestate builder enforces question -> answers -> duration; allow_multiselect is on the
    // shared impl, so it applies cleanly to the already-Ready builder.
    let mut native = serenity::CreatePoll::new()
        .question(question)
        .answers(answers)
        .duration(duration);
    if multiple.unwrap_or(false) {
        native = native.allow_multiselect();
    }

    // Public reply (no `.ephemeral`): the poll must be visible for anyone to vote.
    ctx.send(poise::CreateReply::default().poll(native)).await?;
    Ok(())
}

/// The poll command, for [`crate::commands::all`].
pub fn commands() -> Vec<poise::Command<Data, Error>> {
    vec![poll()]
}

#[cfg(test)]
mod tests {
    use super::*;

    // --- validate_poll_question ---

    #[test]
    fn test_validate_poll_question_typical_returns_trimmed() {
        assert_eq!(
            validate_poll_question("  Cats or dogs?  "),
            Ok("Cats or dogs?".to_string())
        );
    }

    #[test]
    fn test_validate_poll_question_empty_returns_empty_question() {
        assert_eq!(validate_poll_question(""), Err(PollError::EmptyQuestion));
    }

    #[test]
    fn test_validate_poll_question_whitespace_only_returns_empty_question() {
        assert_eq!(validate_poll_question("   "), Err(PollError::EmptyQuestion));
    }

    #[test]
    fn test_validate_poll_question_300_chars_returns_ok() {
        let q = "x".repeat(300);
        assert_eq!(validate_poll_question(&q), Ok(q.clone()));
    }

    #[test]
    fn test_validate_poll_question_301_chars_returns_too_long() {
        let q = "x".repeat(301);
        assert_eq!(validate_poll_question(&q), Err(PollError::QuestionTooLong));
    }

    #[test]
    fn test_validate_poll_question_counts_chars_not_bytes() {
        // 300 multi-byte chars is 1200 bytes but a valid 300-codepoint question.
        let q = "é".repeat(300);
        assert_eq!(validate_poll_question(&q), Ok(q.clone()));
    }

    // --- parse_poll_options ---

    #[test]
    fn test_parse_poll_options_two_options_returns_both() {
        assert_eq!(
            parse_poll_options("Yes | No"),
            Ok(vec!["Yes".to_string(), "No".to_string()])
        );
    }

    #[test]
    fn test_parse_poll_options_trims_each_option() {
        assert_eq!(
            parse_poll_options("  Pizza |Sushi  |  Tacos"),
            Ok(vec![
                "Pizza".to_string(),
                "Sushi".to_string(),
                "Tacos".to_string()
            ])
        );
    }

    #[test]
    fn test_parse_poll_options_drops_blank_entries() {
        // Trailing and doubled separators yield blanks that are dropped, not counted.
        assert_eq!(
            parse_poll_options("A || B |"),
            Ok(vec!["A".to_string(), "B".to_string()])
        );
    }

    #[test]
    fn test_parse_poll_options_ten_options_returns_ok() {
        let input = (1..=10)
            .map(|n| n.to_string())
            .collect::<Vec<_>>()
            .join(" | ");
        assert_eq!(parse_poll_options(&input).map(|v| v.len()), Ok(10));
    }

    #[test]
    fn test_parse_poll_options_one_option_returns_too_few() {
        assert_eq!(parse_poll_options("OnlyOne"), Err(PollError::TooFewOptions));
    }

    #[test]
    fn test_parse_poll_options_comma_separated_reads_as_one_option() {
        // A comma is NOT the separator; "A, B, C" is a single option, hence too few.
        assert_eq!(parse_poll_options("A, B, C"), Err(PollError::TooFewOptions));
    }

    #[test]
    fn test_parse_poll_options_empty_returns_too_few() {
        assert_eq!(parse_poll_options(""), Err(PollError::TooFewOptions));
    }

    #[test]
    fn test_parse_poll_options_eleven_options_returns_too_many() {
        let input = (1..=11)
            .map(|n| n.to_string())
            .collect::<Vec<_>>()
            .join(" | ");
        assert_eq!(parse_poll_options(&input), Err(PollError::TooManyOptions));
    }

    #[test]
    fn test_parse_poll_options_option_55_chars_returns_ok() {
        let long = "x".repeat(55);
        let input = format!("{long} | short");
        assert_eq!(
            parse_poll_options(&input),
            Ok(vec![long.clone(), "short".to_string()])
        );
    }

    #[test]
    fn test_parse_poll_options_option_56_chars_returns_too_long() {
        let input = format!("{} | ok", "x".repeat(56));
        assert_eq!(parse_poll_options(&input), Err(PollError::OptionTooLong));
    }

    #[test]
    fn test_parse_poll_options_counts_chars_not_bytes() {
        // 55 multi-byte chars: 110 bytes but a valid 55-codepoint option.
        let input = format!("{} | ok", "é".repeat(55));
        assert!(parse_poll_options(&input).is_ok());
    }

    // --- parse_poll_duration ---

    #[test]
    fn test_parse_poll_duration_none_returns_24h_default() {
        assert_eq!(
            parse_poll_duration(None),
            Ok(Duration::from_secs(24 * 3_600))
        );
    }

    #[test]
    fn test_parse_poll_duration_1h_returns_ok() {
        assert_eq!(
            parse_poll_duration(Some("1h")),
            Ok(Duration::from_secs(3_600))
        );
    }

    #[test]
    fn test_parse_poll_duration_2d_returns_ok() {
        assert_eq!(
            parse_poll_duration(Some("2d")),
            Ok(Duration::from_secs(2 * 86_400))
        );
    }

    #[test]
    fn test_parse_poll_duration_32d_max_returns_ok() {
        assert_eq!(
            parse_poll_duration(Some("32d")),
            Ok(Duration::from_secs(32 * 86_400))
        );
    }

    #[test]
    fn test_parse_poll_duration_768h_max_returns_ok() {
        // 768 hours == 32 days exactly, expressed in hours.
        assert_eq!(
            parse_poll_duration(Some("768h")),
            Ok(Duration::from_secs(768 * 3_600))
        );
    }

    #[test]
    fn test_parse_poll_duration_outer_whitespace_is_trimmed() {
        assert_eq!(
            parse_poll_duration(Some("  6h ")),
            Ok(Duration::from_secs(6 * 3_600))
        );
    }

    #[test]
    fn test_parse_poll_duration_sub_hour_returns_invalid() {
        // 30 minutes is below the 1-hour floor (would round to 0h at Discord).
        assert_eq!(
            parse_poll_duration(Some("30m")),
            Err(PollError::InvalidDuration)
        );
    }

    #[test]
    fn test_parse_poll_duration_non_whole_hour_snaps_down() {
        // 90 minutes is >= the 1-hour floor but not a whole hour; it snaps to exactly 1h so the
        // returned Duration matches the runtime Discord will actually use.
        assert_eq!(
            parse_poll_duration(Some("90m")),
            Ok(Duration::from_secs(3_600))
        );
    }

    #[test]
    fn test_parse_poll_duration_33d_returns_invalid() {
        assert_eq!(
            parse_poll_duration(Some("33d")),
            Err(PollError::InvalidDuration)
        );
    }

    #[test]
    fn test_parse_poll_duration_769h_returns_invalid() {
        // 32 days + 1 hour, just over the ceiling.
        assert_eq!(
            parse_poll_duration(Some("769h")),
            Err(PollError::InvalidDuration)
        );
    }

    #[test]
    fn test_parse_poll_duration_zero_returns_invalid() {
        assert_eq!(
            parse_poll_duration(Some("0h")),
            Err(PollError::InvalidDuration)
        );
    }

    #[test]
    fn test_parse_poll_duration_empty_returns_invalid() {
        assert_eq!(
            parse_poll_duration(Some("")),
            Err(PollError::InvalidDuration)
        );
    }

    #[test]
    fn test_parse_poll_duration_missing_unit_returns_invalid() {
        assert_eq!(
            parse_poll_duration(Some("6")),
            Err(PollError::InvalidDuration)
        );
    }

    #[test]
    fn test_parse_poll_duration_unknown_unit_returns_invalid() {
        assert_eq!(
            parse_poll_duration(Some("6w")),
            Err(PollError::InvalidDuration)
        );
    }

    #[test]
    fn test_parse_poll_duration_non_integer_returns_invalid() {
        assert_eq!(
            parse_poll_duration(Some("abc")),
            Err(PollError::InvalidDuration)
        );
    }

    #[test]
    fn test_parse_poll_duration_decimal_returns_invalid() {
        assert_eq!(
            parse_poll_duration(Some("1.5h")),
            Err(PollError::InvalidDuration)
        );
    }

    #[test]
    fn test_parse_poll_duration_overflow_returns_invalid_without_panic() {
        let result = parse_poll_duration(Some("99999999999999999999d"));
        assert_eq!(result, Err(PollError::InvalidDuration));
    }
}
