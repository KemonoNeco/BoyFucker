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
    let magnitude: u64 = magnitude.parse().map_err(|_| ModError::InvalidDuration)?;

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

// ---------------------------------------------------------------------------
// Command handlers (I/O glue — verified by compile + live run, not unit tests).
// Each parses args, runs the pure validator(s) above, performs the serenity HTTP
// action, and replies. The pure functions hold the logic; these are the wiring.
// ---------------------------------------------------------------------------

use crate::access::moderation_access_check;
use crate::{Context, Data, Error};
use poise::serenity_prelude as serenity;

/// Current UNIX time in seconds (used to compute timeout expiry timestamps).
fn now_unix_secs() -> Result<i64, Error> {
    Ok(std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)?
        .as_secs() as i64)
}

/// Reply to the command with a [`ModError`]'s user-facing Display message.
async fn reply_err(ctx: Context<'_>, err: ModError) -> Result<(), Error> {
    ctx.say(err.to_string()).await?;
    Ok(())
}

/// Resolve the live facts for a moderation attempt and run [`check_moderation_allowed`].
///
/// Role positions come from the cached guild; the actor's member is fetched if not cached.
/// The cache borrow is confined to a sync block so it is never held across an `.await`.
async fn authorize(
    ctx: &Context<'_>,
    target: &serenity::Member,
) -> Result<Result<(), ModError>, Error> {
    let guild_id = ctx
        .guild_id()
        .ok_or("this command can only be used in a guild")?;
    let actor_id = ctx.author().id;
    let bot_id = ctx.serenity_context().cache.current_user().id;

    // Fetch the actor's member (for their role positions) before touching the cache.
    let actor_member = guild_id.member(ctx.serenity_context(), actor_id).await?;

    let (actor_top_role, target_top_role, owner_id) = {
        let guild = ctx.guild().ok_or("guild is not available in the cache")?;
        let highest = |roles: &[serenity::RoleId]| -> i64 {
            roles
                .iter()
                .filter_map(|r| guild.roles.get(r))
                .map(|r| r.position as i64)
                .max()
                .unwrap_or(0)
        };
        (
            highest(&actor_member.roles),
            highest(&target.roles),
            guild.owner_id,
        )
    };

    Ok(check_moderation_allowed(ModCheck {
        actor_id: actor_id.get(),
        target_id: target.user.id.get(),
        bot_id: bot_id.get(),
        actor_is_owner: actor_id == owner_id,
        target_is_owner: target.user.id == owner_id,
        actor_top_role,
        target_top_role,
    }))
}

/// Bulk-delete recent messages in the current channel (1-100).
#[poise::command(
    slash_command,
    required_permissions = "MANAGE_MESSAGES",
    guild_only,
    check = "moderation_access_check"
)]
pub async fn purge(
    ctx: Context<'_>,
    #[description = "How many recent messages to delete (1-100)"] count: i64,
) -> Result<(), Error> {
    let count = match validate_purge_count(count) {
        Ok(n) => n,
        Err(e) => return reply_err(ctx, e).await,
    };
    let channel = ctx.channel_id();
    let messages = channel
        .messages(ctx.http(), serenity::GetMessages::new().limit(count))
        .await?;
    let ids: Vec<serenity::MessageId> = messages.iter().map(|m| m.id).collect();
    match ids.len() {
        0 => {
            ctx.say("No messages to delete.").await?;
        }
        1 => {
            channel.delete_message(ctx.http(), ids[0]).await?;
            ctx.say("Deleted 1 message.").await?;
        }
        n => {
            // Bulk delete only covers messages newer than 14 days; older ones surface an error.
            channel.delete_messages(ctx.http(), &ids).await?;
            ctx.say(format!("Deleted {n} messages.")).await?;
        }
    }
    Ok(())
}

/// Kick a member from the guild.
#[poise::command(
    slash_command,
    required_permissions = "KICK_MEMBERS",
    guild_only,
    check = "moderation_access_check"
)]
pub async fn kick(
    ctx: Context<'_>,
    #[description = "Member to kick"] member: serenity::Member,
    #[description = "Reason"] reason: Option<String>,
) -> Result<(), Error> {
    if let Err(e) = authorize(&ctx, &member).await? {
        return reply_err(ctx, e).await;
    }
    let reason = reason.unwrap_or_else(|| "No reason provided".to_string());
    member
        .kick_with_reason(ctx.serenity_context(), &reason)
        .await?;
    ctx.say(format!("Kicked {} ({reason}).", member.user.name))
        .await?;
    Ok(())
}

/// Ban a member, optionally deleting their recent messages (0-7 days).
#[poise::command(
    slash_command,
    required_permissions = "BAN_MEMBERS",
    guild_only,
    check = "moderation_access_check"
)]
pub async fn ban(
    ctx: Context<'_>,
    #[description = "Member to ban"] member: serenity::Member,
    #[description = "Days of their messages to delete (0-7)"] delete_message_days: Option<i64>,
    #[description = "Reason"] reason: Option<String>,
) -> Result<(), Error> {
    let days = delete_message_days.unwrap_or(0);
    // Validate the 0-7 range via the pure helper; serenity's ban takes DAYS (u8), so we
    // pass `days` directly (the helper's seconds result is only used to gate the range).
    if let Err(e) = validate_ban_delete_days(days) {
        return reply_err(ctx, e).await;
    }
    if let Err(e) = authorize(&ctx, &member).await? {
        return reply_err(ctx, e).await;
    }
    let reason = reason.unwrap_or_else(|| "No reason provided".to_string());
    let guild_id = ctx
        .guild_id()
        .ok_or("this command can only be used in a guild")?;
    guild_id
        .ban_with_reason(ctx.http(), member.user.id, days as u8, &reason)
        .await?;
    ctx.say(format!("Banned {} ({reason}).", member.user.name))
        .await?;
    Ok(())
}

/// Unban a user by ID.
#[poise::command(
    slash_command,
    required_permissions = "BAN_MEMBERS",
    guild_only,
    check = "moderation_access_check"
)]
pub async fn unban(
    ctx: Context<'_>,
    #[description = "User to unban (ID)"] user: serenity::User,
) -> Result<(), Error> {
    let guild_id = ctx
        .guild_id()
        .ok_or("this command can only be used in a guild")?;
    guild_id.unban(ctx.http(), user.id).await?;
    ctx.say(format!("Unbanned {}.", user.name)).await?;
    Ok(())
}

/// Timeout (mute) a member for a duration like `10m`, `2h`, `7d` (max 28 days).
#[poise::command(
    slash_command,
    required_permissions = "MODERATE_MEMBERS",
    guild_only,
    check = "moderation_access_check"
)]
pub async fn mute(
    ctx: Context<'_>,
    #[description = "Member to mute"] mut member: serenity::Member,
    #[description = "Duration, e.g. 30s, 10m, 2h, 7d (max 28d)"] duration: String,
) -> Result<(), Error> {
    let duration = match parse_timeout_duration(&duration) {
        Ok(d) => d,
        Err(e) => return reply_err(ctx, e).await,
    };
    if let Err(e) = authorize(&ctx, &member).await? {
        return reply_err(ctx, e).await;
    }
    let until = now_unix_secs()? + duration.as_secs() as i64;
    let timestamp = serenity::Timestamp::from_unix_timestamp(until)?;
    member
        .disable_communication_until_datetime(ctx.serenity_context(), timestamp)
        .await?;
    ctx.say(format!("Muted {} until <t:{until}:f>.", member.user.name))
        .await?;
    Ok(())
}

/// Clear a member's timeout (unmute).
#[poise::command(
    slash_command,
    required_permissions = "MODERATE_MEMBERS",
    guild_only,
    check = "moderation_access_check"
)]
pub async fn unmute(
    ctx: Context<'_>,
    #[description = "Member to unmute"] mut member: serenity::Member,
) -> Result<(), Error> {
    member.enable_communication(ctx.serenity_context()).await?;
    ctx.say(format!("Unmuted {}.", member.user.name)).await?;
    Ok(())
}

/// All moderation slash commands, for registration in [`crate::commands::all`].
pub fn commands() -> Vec<poise::Command<Data, Error>> {
    vec![purge(), kick(), ban(), unban(), mute(), unmute()]
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
        assert_eq!(parse_timeout_duration(" 30s "), Ok(Duration::from_secs(30)));
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
        assert_eq!(
            parse_timeout_duration("29d"),
            Err(ModError::DurationTooLong)
        );
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
        assert_eq!(
            parse_timeout_duration("   "),
            Err(ModError::InvalidDuration)
        );
    }

    #[test]
    fn test_parse_timeout_duration_missing_unit_returns_invalid() {
        assert_eq!(parse_timeout_duration("10"), Err(ModError::InvalidDuration));
    }

    #[test]
    fn test_parse_timeout_duration_unknown_unit_returns_invalid() {
        assert_eq!(
            parse_timeout_duration("10x"),
            Err(ModError::InvalidDuration)
        );
    }

    #[test]
    fn test_parse_timeout_duration_non_integer_returns_invalid() {
        assert_eq!(
            parse_timeout_duration("abc"),
            Err(ModError::InvalidDuration)
        );
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
        assert_eq!(
            parse_timeout_duration("-5m"),
            Err(ModError::InvalidDuration)
        );
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
    fn test_parse_timeout_duration_one_second_returns_one_second() {
        // The smallest accepted magnitude. Pins the zero-reject boundary at exactly 0,
        // so a `magnitude == 0` → `magnitude <= 1` mutation rejects this and fails.
        assert_eq!(parse_timeout_duration("1s"), Ok(Duration::from_secs(1)));
    }

    #[test]
    fn test_parse_timeout_duration_unit_only_returns_invalid() {
        // A bare unit with no magnitude: the magnitude substring is empty and must
        // fail to parse rather than be treated as any default.
        assert_eq!(parse_timeout_duration("m"), Err(ModError::InvalidDuration));
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

    // --- precedence pinning: the rule order in the doc comment is a contract ---
    // The cases below force two rules to both apply at once, so each one fails
    // only if its higher-precedence rule were demoted below the lower one.

    #[test]
    fn test_check_moderation_allowed_owner_targets_self_returns_cannot_target_self() {
        // Guild owner runs the action on themselves: rule 1 (self) must win over both
        // rule 3 (target is owner) and rule 4 (actor-owner bypass). A swap of rule 1
        // below rule 3 yields CannotTargetOwner; promoting the bypass yields Ok.
        let check = ModCheck {
            actor_id: 1,
            target_id: 1,
            bot_id: 99,
            actor_is_owner: true,
            target_is_owner: true,
            actor_top_role: 10,
            target_top_role: 10,
        };
        assert_eq!(
            check_moderation_allowed(check),
            Err(ModError::CannotTargetSelf)
        );
    }

    #[test]
    fn test_check_moderation_allowed_owner_targets_bot_returns_cannot_target_bot() {
        // Guild owner runs the action on the bot: rule 2 (target is bot) must win over
        // rule 4 (actor-owner bypass). Promoting the bypass above rule 2 yields Ok.
        let check = ModCheck {
            actor_id: 1,
            target_id: 99,
            bot_id: 99,
            actor_is_owner: true,
            target_is_owner: false,
            actor_top_role: 10,
            target_top_role: 1,
        };
        assert_eq!(
            check_moderation_allowed(check),
            Err(ModError::CannotTargetBot)
        );
    }

    #[test]
    fn test_check_moderation_allowed_bot_flagged_owner_returns_cannot_target_bot() {
        // Pins the rule 2 (bot) over rule 3 (owner) adjacency: only a bot/owner swap
        // would turn this into CannotTargetOwner. NOTE: this input combination cannot
        // occur live (a bot can't own a guild) — the test exists solely to lock the
        // documented precedence, not to cover a reachable path.
        let check = ModCheck {
            actor_id: 1,
            target_id: 99,
            bot_id: 99,
            actor_is_owner: false,
            target_is_owner: true,
            actor_top_role: 5,
            target_top_role: 1,
        };
        assert_eq!(
            check_moderation_allowed(check),
            Err(ModError::CannotTargetBot)
        );
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
