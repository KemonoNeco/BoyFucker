//! Moderation command helpers: pure validation/authorization functions.
//!
//! These are the locked-contract helpers backing the kick/ban/timeout/purge commands. They are
//! pure (no I/O) so they can be unit-tested in isolation. Implementations are stubbed for the
//! TDD red phase — every body panics via `unimplemented!()` until the implementation author fills
//! them in during the green phase.

use std::time::Duration;

// The moderation validators/parser return the SINGLE canonical ModError defined in
// `src/error.rs`. There must NOT be a duplicate local ModError here; the canonical
// Display strings (e.g. "the maximum is 28 days", "1 and 100 messages") are the
// user-facing contract these helpers feed into.
use crate::error::ModError;

/// Resolved facts about a moderation attempt, used by [`check_moderation_allowed`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ModCheck {
    pub actor_id: u64,
    pub target_id: u64,
    pub bot_id: u64,
    pub actor_is_owner: bool,
    pub target_is_owner: bool,
    pub actor_top_role: i64,
    pub target_top_role: i64,
}

/// Validate a Discord bulk-delete purge count, returning the value as `u8` on success.
///
/// Valid range is the inclusive `1..=100` Discord bulk-delete window. The range check must occur
/// on the wide `i64` input before any cast to `u8`.
pub fn validate_purge_count(count: i64) -> Result<u8, ModError> {
    // Range-check the wide i64 input BEFORE any cast, so out-of-range values
    // (negatives, i64 extremes, or values like 300 that would alias into range
    // under a truncating cast) are rejected rather than wrapped.
    if (1..=100).contains(&count) {
        Ok(count as u8)
    } else {
        Err(ModError::InvalidPurgeCount)
    }
}

/// Validate a ban message-delete window in days, returning the equivalent seconds as `u32`.
///
/// Valid range is the inclusive `0..=7` range; the result is `days * 86_400` seconds.
pub fn validate_ban_delete_days(days: i64) -> Result<u32, ModError> {
    // Range-check the wide i64 input BEFORE any cast or multiply, so negatives,
    // i64 extremes, and values like 2^32 (which would alias to 0 under a u32 cast)
    // are rejected. After the check, days is in 0..=7 so days * 86_400 fits in u32.
    if (0..=7).contains(&days) {
        Ok(days as u32 * 86_400)
    } else {
        Err(ModError::InvalidBanDeleteDays)
    }
}

/// Parse a timeout duration token like `"30s"`, `"10m"`, `"2h"`, `"7d"` into a [`Duration`].
///
/// Outer whitespace is trimmed. The magnitude must be a positive integer and the unit one of
/// `s`/`m`/`h`/`d`. The computed total must not exceed the 28-day (2_419_200s) maximum.
pub fn parse_timeout_duration(input: &str) -> Result<Duration, ModError> {
    /// Discord's inclusive maximum timeout: 28 days expressed in seconds.
    const MAX_TIMEOUT_SECS: u64 = 28 * 86_400; // 2_419_200

    let trimmed = input.trim();

    // The unit suffix is the final character; split it off the magnitude. An empty
    // (or whitespace-only) input has no characters and is malformed.
    let (magnitude, unit) = match trimmed.char_indices().next_back() {
        Some((idx, unit_char)) => (&trimmed[..idx], unit_char),
        None => return Err(ModError::InvalidDuration),
    };

    // Map the unit suffix to its seconds-per-unit factor. Unknown units are malformed.
    let seconds_per_unit: u64 = match unit {
        's' => 1,
        'm' => 60,
        'h' => 3_600,
        'd' => 86_400,
        _ => return Err(ModError::InvalidDuration),
    };

    // The magnitude must be a non-negative integer. Parsing as u64 rejects negatives,
    // decimals, non-numeric text, and a bare missing magnitude; a too-wide literal that
    // overflows u64 also fails here and surfaces as InvalidDuration (never a panic).
    let magnitude: u64 = magnitude
        .parse()
        .map_err(|_| ModError::InvalidDuration)?;

    // A zero-magnitude timeout is meaningless regardless of unit.
    if magnitude == 0 {
        return Err(ModError::InvalidDuration);
    }

    // Compute total seconds with checked arithmetic so a huge magnitude cannot panic.
    // Overflow of the multiply means the value is astronomically large; the syntax was
    // valid but no representable duration satisfies it, so treat it as too long.
    let total_secs = magnitude
        .checked_mul(seconds_per_unit)
        .ok_or(ModError::DurationTooLong)?;

    // The 28-day limit is enforced on the computed total seconds, not the per-unit
    // magnitude, so "40320m" (== 28d) is accepted and "40321m" is too long.
    if total_secs > MAX_TIMEOUT_SECS {
        return Err(ModError::DurationTooLong);
    }

    Ok(Duration::from_secs(total_secs))
}

/// Authorize a moderation action over resolved facts, returning `Ok(())` when allowed.
///
/// Evaluation order: (1) cannot target self, (2) cannot target bot, (3) cannot target owner,
/// (4) guild-owner actor bypass, (5) strict role hierarchy.
pub fn check_moderation_allowed(check: ModCheck) -> Result<(), ModError> {
    // Rules are evaluated in strict precedence order; the first matching rule wins.

    // Rule 1: cannot target self (wins over rule 2 when actor == target == bot).
    if check.target_id == check.actor_id {
        return Err(ModError::CannotTargetSelf);
    }

    // Rule 2: cannot target the bot.
    if check.target_id == check.bot_id {
        return Err(ModError::CannotTargetBot);
    }

    // Rule 3: cannot target the guild owner (wins over rule 4 when both are owners).
    if check.target_is_owner {
        return Err(ModError::CannotTargetOwner);
    }

    // Rule 4: a guild-owner actor bypasses the hierarchy check entirely.
    if check.actor_is_owner {
        return Ok(());
    }

    // Rule 5: the actor's top role must be strictly higher than the target's.
    if check.actor_top_role > check.target_top_role {
        Ok(())
    } else {
        Err(ModError::InsufficientHierarchy)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // --- validate_purge_count ---

    #[test]
    fn test_validate_purge_count_zero_returns_invalid() {
        assert_eq!(validate_purge_count(0), Err(ModError::InvalidPurgeCount));
    }

    #[test]
    fn test_validate_purge_count_one_returns_ok_one() {
        assert_eq!(validate_purge_count(1), Ok(1u8));
    }

    #[test]
    fn test_validate_purge_count_typical_returns_ok() {
        assert_eq!(validate_purge_count(50), Ok(50u8));
    }

    #[test]
    fn test_validate_purge_count_hundred_returns_ok_hundred() {
        assert_eq!(validate_purge_count(100), Ok(100u8));
    }

    #[test]
    fn test_validate_purge_count_hundred_one_returns_invalid() {
        assert_eq!(validate_purge_count(101), Err(ModError::InvalidPurgeCount));
    }

    #[test]
    fn test_validate_purge_count_negative_returns_invalid() {
        assert_eq!(validate_purge_count(-5), Err(ModError::InvalidPurgeCount));
    }

    #[test]
    fn test_validate_purge_count_i64_extremes_return_invalid() {
        assert_eq!(
            validate_purge_count(i64::MAX),
            Err(ModError::InvalidPurgeCount)
        );
        assert_eq!(
            validate_purge_count(i64::MIN),
            Err(ModError::InvalidPurgeCount)
        );
    }

    #[test]
    fn test_validate_purge_count_aliasing_value_returns_invalid() {
        // 300 as u8 == 44, which is inside 1..=100; range check must precede the cast.
        assert_eq!(validate_purge_count(300), Err(ModError::InvalidPurgeCount));
    }

    // --- validate_ban_delete_days ---

    #[test]
    fn test_validate_ban_delete_days_negative_returns_invalid() {
        assert_eq!(
            validate_ban_delete_days(-1),
            Err(ModError::InvalidBanDeleteDays)
        );
    }

    #[test]
    fn test_validate_ban_delete_days_zero_returns_ok_zero_seconds() {
        assert_eq!(validate_ban_delete_days(0), Ok(0u32));
    }

    #[test]
    fn test_validate_ban_delete_days_typical_returns_ok_seconds() {
        assert_eq!(validate_ban_delete_days(3), Ok(259_200u32));
    }

    #[test]
    fn test_validate_ban_delete_days_seven_returns_ok_week_seconds() {
        assert_eq!(validate_ban_delete_days(7), Ok(604_800u32));
    }

    #[test]
    fn test_validate_ban_delete_days_eight_returns_invalid() {
        assert_eq!(
            validate_ban_delete_days(8),
            Err(ModError::InvalidBanDeleteDays)
        );
    }

    #[test]
    fn test_validate_ban_delete_days_i64_extremes_return_invalid() {
        assert_eq!(
            validate_ban_delete_days(i64::MAX),
            Err(ModError::InvalidBanDeleteDays)
        );
        assert_eq!(
            validate_ban_delete_days(i64::MIN),
            Err(ModError::InvalidBanDeleteDays)
        );
    }

    #[test]
    fn test_validate_ban_delete_days_aliasing_value_returns_invalid() {
        // 2^32 as u32 == 0, which is inside 0..=7; range check must precede the cast.
        assert_eq!(
            validate_ban_delete_days(4_294_967_296),
            Err(ModError::InvalidBanDeleteDays)
        );
    }

    // --- parse_timeout_duration ---

    #[test]
    fn test_parse_timeout_duration_30s_returns_30_seconds() {
        assert_eq!(parse_timeout_duration("30s"), Ok(Duration::from_secs(30)));
    }

    #[test]
    fn test_parse_timeout_duration_10m_returns_600_seconds() {
        assert_eq!(parse_timeout_duration("10m"), Ok(Duration::from_secs(600)));
    }

    #[test]
    fn test_parse_timeout_duration_2h_returns_7200_seconds() {
        assert_eq!(parse_timeout_duration("2h"), Ok(Duration::from_secs(7200)));
    }

    #[test]
    fn test_parse_timeout_duration_7d_returns_604800_seconds() {
        assert_eq!(
            parse_timeout_duration("7d"),
            Ok(Duration::from_secs(604800))
        );
    }

    #[test]
    fn test_parse_timeout_duration_28d_max_returns_ok() {
        assert_eq!(
            parse_timeout_duration("28d"),
            Ok(Duration::from_secs(2_419_200))
        );
    }

    #[test]
    fn test_parse_timeout_duration_outer_whitespace_is_trimmed() {
        assert_eq!(
            parse_timeout_duration(" 30s "),
            Ok(Duration::from_secs(30))
        );
    }

    #[test]
    fn test_parse_timeout_duration_28d_in_minutes_returns_ok() {
        // 40320 minutes == 28 days exactly; the limit is checked on total seconds.
        assert_eq!(
            parse_timeout_duration("40320m"),
            Ok(Duration::from_secs(2_419_200))
        );
    }

    #[test]
    fn test_parse_timeout_duration_29d_returns_too_long() {
        assert_eq!(parse_timeout_duration("29d"), Err(ModError::DurationTooLong));
    }

    #[test]
    fn test_parse_timeout_duration_40321m_returns_too_long() {
        // 28 days + 1 minute, expressed in a non-day unit.
        assert_eq!(
            parse_timeout_duration("40321m"),
            Err(ModError::DurationTooLong)
        );
    }

    #[test]
    fn test_parse_timeout_duration_empty_returns_invalid() {
        assert_eq!(parse_timeout_duration(""), Err(ModError::InvalidDuration));
    }

    #[test]
    fn test_parse_timeout_duration_whitespace_only_returns_invalid() {
        assert_eq!(parse_timeout_duration("   "), Err(ModError::InvalidDuration));
    }

    #[test]
    fn test_parse_timeout_duration_missing_unit_returns_invalid() {
        assert_eq!(parse_timeout_duration("10"), Err(ModError::InvalidDuration));
    }

    #[test]
    fn test_parse_timeout_duration_unknown_unit_returns_invalid() {
        assert_eq!(parse_timeout_duration("10x"), Err(ModError::InvalidDuration));
    }

    #[test]
    fn test_parse_timeout_duration_non_integer_returns_invalid() {
        assert_eq!(parse_timeout_duration("abc"), Err(ModError::InvalidDuration));
    }

    #[test]
    fn test_parse_timeout_duration_decimal_returns_invalid() {
        assert_eq!(
            parse_timeout_duration("1.5h"),
            Err(ModError::InvalidDuration)
        );
    }

    #[test]
    fn test_parse_timeout_duration_negative_returns_invalid() {
        assert_eq!(parse_timeout_duration("-5m"), Err(ModError::InvalidDuration));
    }

    #[test]
    fn test_parse_timeout_duration_zero_minutes_returns_invalid() {
        assert_eq!(parse_timeout_duration("0m"), Err(ModError::InvalidDuration));
    }

    #[test]
    fn test_parse_timeout_duration_zero_seconds_returns_invalid() {
        assert_eq!(parse_timeout_duration("0s"), Err(ModError::InvalidDuration));
    }

    #[test]
    fn test_parse_timeout_duration_overflow_returns_err_without_panic() {
        // 20-digit magnitude overflows the u64 parse and the *86400 multiply; must error, not panic.
        let result = parse_timeout_duration("99999999999999999999d");
        assert!(result.is_err());
    }

    // --- check_moderation_allowed ---

    #[test]
    fn test_check_moderation_allowed_self_target_returns_cannot_target_self() {
        let check = ModCheck {
            actor_id: 1,
            target_id: 1,
            bot_id: 99,
            actor_is_owner: false,
            target_is_owner: false,
            actor_top_role: 5,
            target_top_role: 1,
        };
        assert_eq!(
            check_moderation_allowed(check),
            Err(ModError::CannotTargetSelf)
        );
    }

    #[test]
    fn test_check_moderation_allowed_bot_target_returns_cannot_target_bot() {
        let check = ModCheck {
            actor_id: 1,
            target_id: 99,
            bot_id: 99,
            actor_is_owner: false,
            target_is_owner: false,
            actor_top_role: 5,
            target_top_role: 1,
        };
        assert_eq!(
            check_moderation_allowed(check),
            Err(ModError::CannotTargetBot)
        );
    }

    #[test]
    fn test_check_moderation_allowed_owner_target_returns_cannot_target_owner() {
        let check = ModCheck {
            actor_id: 1,
            target_id: 2,
            bot_id: 99,
            actor_is_owner: false,
            target_is_owner: true,
            actor_top_role: 5,
            target_top_role: 1,
        };
        assert_eq!(
            check_moderation_allowed(check),
            Err(ModError::CannotTargetOwner)
        );
    }

    #[test]
    fn test_check_moderation_allowed_actor_owner_bypasses_hierarchy_returns_ok() {
        let check = ModCheck {
            actor_id: 1,
            target_id: 2,
            bot_id: 99,
            actor_is_owner: true,
            target_is_owner: false,
            actor_top_role: 1,
            target_top_role: 10,
        };
        assert_eq!(check_moderation_allowed(check), Ok(()));
    }

    #[test]
    fn test_check_moderation_allowed_owner_targets_owner_returns_cannot_target_owner() {
        let check = ModCheck {
            actor_id: 1,
            target_id: 2,
            bot_id: 99,
            actor_is_owner: true,
            target_is_owner: true,
            actor_top_role: 10,
            target_top_role: 1,
        };
        assert_eq!(
            check_moderation_allowed(check),
            Err(ModError::CannotTargetOwner)
        );
    }

    #[test]
    fn test_check_moderation_allowed_self_and_bot_returns_cannot_target_self() {
        let check = ModCheck {
            actor_id: 99,
            target_id: 99,
            bot_id: 99,
            actor_is_owner: false,
            target_is_owner: false,
            actor_top_role: 5,
            target_top_role: 1,
        };
        assert_eq!(
            check_moderation_allowed(check),
            Err(ModError::CannotTargetSelf)
        );
    }

    #[test]
    fn test_check_moderation_allowed_equal_roles_returns_insufficient_hierarchy() {
        let check = ModCheck {
            actor_id: 1,
            target_id: 2,
            bot_id: 99,
            actor_is_owner: false,
            target_is_owner: false,
            actor_top_role: 5,
            target_top_role: 5,
        };
        assert_eq!(
            check_moderation_allowed(check),
            Err(ModError::InsufficientHierarchy)
        );
    }

    #[test]
    fn test_check_moderation_allowed_actor_lower_role_returns_insufficient_hierarchy() {
        let check = ModCheck {
            actor_id: 1,
            target_id: 2,
            bot_id: 99,
            actor_is_owner: false,
            target_is_owner: false,
            actor_top_role: 3,
            target_top_role: 8,
        };
        assert_eq!(
            check_moderation_allowed(check),
            Err(ModError::InsufficientHierarchy)
        );
    }

    #[test]
    fn test_check_moderation_allowed_actor_higher_role_returns_ok() {
        let check = ModCheck {
            actor_id: 1,
            target_id: 2,
            bot_id: 99,
            actor_is_owner: false,
            target_is_owner: false,
            actor_top_role: 6,
            target_top_role: 5,
        };
        assert_eq!(check_moderation_allowed(check), Ok(()));
    }

    // --- canonical ModError Display discrimination (crate::error::ModError) ---

    // WORK_UNIT_ID: wu-mod-fix-parse-timeout-29d-error-mentions-28
    #[test]
    fn test_parse_timeout_duration_29d_error_mentions_28_day_cap() {
        let rendered = parse_timeout_duration("29d").unwrap_err().to_string();
        assert!(
            rendered.contains("28"),
            "expected canonical DurationTooLong Display to mention the 28-day cap, got: {rendered}"
        );
    }

    // WORK_UNIT_ID: wu-mod-fix-validate-purge-count-error-mentions-messages
    #[test]
    fn test_validate_purge_count_invalid_error_mentions_messages() {
        let rendered = validate_purge_count(0).unwrap_err().to_string();
        assert!(
            rendered.contains("messages"),
            "expected canonical InvalidPurgeCount Display to mention 'messages', got: {rendered}"
        );
    }

    // WORK_UNIT_ID: wu-mod-fix-validate-ban-delete-days-error-mentions-deletion
    #[test]
    fn test_validate_ban_delete_days_invalid_error_mentions_deletion() {
        let rendered = validate_ban_delete_days(8).unwrap_err().to_string();
        assert!(
            rendered.contains("deletion"),
            "expected canonical InvalidBanDeleteDays Display to mention 'deletion', got: {rendered}"
        );
    }

    // WORK_UNIT_ID: wu-mod-fix-check-moderation-allowed-owner-error-mentions-moderate
    #[test]
    fn test_check_moderation_allowed_owner_target_error_mentions_moderate() {
        let check = ModCheck {
            actor_id: 1,
            target_id: 2,
            bot_id: 99,
            actor_is_owner: false,
            target_is_owner: true,
            actor_top_role: 5,
            target_top_role: 1,
        };
        let rendered = check_moderation_allowed(check).unwrap_err().to_string();
        assert!(
            rendered.contains("moderate"),
            "expected canonical CannotTargetOwner Display to mention 'moderate', got: {rendered}"
        );
    }
}
