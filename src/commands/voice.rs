//! `/join` command: have the bot join a voice channel (no audio — just presence, to keep a
//! channel active; a foundation for future voice features).
//!
//! The pure, unit-tested piece is target resolution: an explicitly-chosen channel wins, else the
//! channel the invoker is currently connected to, else an error. The handler is thin glue that
//! reads the invoker's live voice state from the cache and asks songbird to join.

use crate::error::VcError;

/// Resolve which voice channel `/join` should target, as a raw channel-id.
///
/// Precedence: an explicitly-chosen channel wins; otherwise fall back to the channel the invoker
/// is currently connected to; if neither is available there is nothing to join.
pub fn resolve_join_target(
    explicit_channel: Option<u64>,
    invoker_voice_channel: Option<u64>,
) -> Result<u64, VcError> {
    explicit_channel
        .or(invoker_voice_channel)
        .ok_or(VcError::NoTargetChannel)
}

// ---------------------------------------------------------------------------
// Command handler (I/O glue — verified by compile + live run, not unit tests).
// Resolves the target channel via the pure helper above, then asks songbird to
// join it at the gateway level (presence only; no audio driver is configured).
// ---------------------------------------------------------------------------

use crate::{Context, Data, Error};
use poise::serenity_prelude as serenity;

/// Join a voice channel (the bot just sits there, keeping it active).
///
/// With no `channel`, joins the voice channel you're currently in.
#[poise::command(slash_command, guild_only)]
pub async fn join(
    ctx: Context<'_>,
    #[description = "Voice channel to join (defaults to the one you're in)"]
    #[channel_types("Voice")]
    channel: Option<serenity::GuildChannel>,
) -> Result<(), Error> {
    let guild_id = ctx
        .guild_id()
        .ok_or("this command can only be used in a guild")?;

    // Read the invoker's current voice channel from the cache. The cache borrow is confined to a
    // sync block so it is never held across an `.await` (it isn't `Send`).
    let invoker_vc: Option<u64> = {
        let guild = ctx.guild().ok_or("guild is not available in the cache")?;
        guild
            .voice_states
            .get(&ctx.author().id)
            .and_then(|vs| vs.channel_id)
            .map(serenity::ChannelId::get)
    };
    let explicit = channel.as_ref().map(|c| c.id.get());

    let target = match resolve_join_target(explicit, invoker_vc) {
        Ok(id) => serenity::ChannelId::new(id),
        Err(e) => {
            ctx.send(
                poise::CreateReply::default()
                    .content(e.to_string())
                    .ephemeral(true),
            )
            .await?;
            return Ok(());
        }
    };

    let manager = songbird::get(ctx.serenity_context())
        .await
        .ok_or("voice manager was not initialised")?;

    // `join_gateway` sends the gateway voice-state update (the bot appears in the channel) without
    // opening an audio connection — exactly the presence we want.
    match manager.join_gateway(guild_id, target).await {
        Ok(_) => ctx.say(format!("Joined <#{}>.", target.get())).await?,
        Err(e) => ctx.say(format!("Couldn't join that channel: {e}")).await?,
    };
    Ok(())
}

/// The voice commands, for [`crate::commands::all`].
pub fn commands() -> Vec<poise::Command<Data, Error>> {
    vec![join()]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_resolve_join_target_explicit_only_returns_explicit() {
        assert_eq!(resolve_join_target(Some(42), None), Ok(42));
    }

    #[test]
    fn test_resolve_join_target_invoker_only_returns_invoker() {
        assert_eq!(resolve_join_target(None, Some(7)), Ok(7));
    }

    #[test]
    fn test_resolve_join_target_explicit_wins_over_invoker() {
        // Distinct values prove precedence: the explicit choice must win, not the invoker's VC.
        assert_eq!(resolve_join_target(Some(42), Some(7)), Ok(42));
    }

    #[test]
    fn test_resolve_join_target_neither_returns_no_target_channel() {
        assert_eq!(
            resolve_join_target(None, None),
            Err(VcError::NoTargetChannel)
        );
    }
}
