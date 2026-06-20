//! `/proxy link|unlink|list` route management + an owner-only `/proxytest` inbound injector.
//!
//! Thin glue: the commands resolve live data and call the `crate::proxy` store / relay. Expected
//! user-facing failures are modeled as [`ProxyError`] and replied as their Display message; any
//! other error propagates to poise.

use crate::error::ProxyError;
use crate::proxy::{Platform, RemoteMessage, routes, webhook};
use crate::{Context, Data, Error};
use poise::serenity_prelude as serenity;

/// Reply with a [`ProxyError`]'s message if `e` is one, else propagate the error.
async fn reply_or_propagate(ctx: Context<'_>, e: Error, ephemeral: bool) -> Result<(), Error> {
    match e.downcast_ref::<ProxyError>() {
        Some(pe) => {
            ctx.send(
                poise::CreateReply::default()
                    .content(pe.to_string())
                    .ephemeral(ephemeral),
            )
            .await?;
            Ok(())
        }
        None => Err(e),
    }
}

/// Manage channel proxy routes (Telegram ⇄ Discord). Parent group — use a subcommand.
#[poise::command(
    slash_command,
    guild_only,
    subcommands("link", "unlink", "list"),
    subcommand_required
)]
pub async fn proxy(_ctx: Context<'_>) -> Result<(), Error> {
    Ok(())
}

/// Link a Discord channel to a remote Telegram chat.
#[poise::command(slash_command, guild_only, required_permissions = "MANAGE_WEBHOOKS")]
pub async fn link(
    ctx: Context<'_>,
    #[description = "Discord channel to bridge"]
    #[channel_types("Text")]
    channel: serenity::GuildChannel,
    #[description = "Remote Telegram chat id to link it to"] remote_chat_id: String,
) -> Result<(), Error> {
    let guild_id = ctx
        .guild_id()
        .ok_or("this command can only be used in a guild")?
        .get();
    let remote = remote_chat_id.trim();
    let by = ctx.author().id.get();
    match routes::link(
        &ctx.data().db,
        guild_id,
        channel.id.get(),
        Platform::Telegram,
        remote,
        by,
    )
    .await
    {
        Ok(()) => {
            // remote_chat_id is free-form user input; suppress mentions so a value like
            // "@everyone" can't ping when echoed back (backticks alone don't stop @everyone).
            ctx.send(
                poise::CreateReply::default()
                    .content(format!(
                        "✅ Linked <#{}> ⇄ {} chat `{remote}`.",
                        channel.id.get(),
                        Platform::Telegram.label()
                    ))
                    .allowed_mentions(serenity::CreateAllowedMentions::new()),
            )
            .await?;
            Ok(())
        }
        Err(e) => reply_or_propagate(ctx, e, false).await,
    }
}

/// Unlink a Discord channel from its Telegram route.
#[poise::command(slash_command, guild_only, required_permissions = "MANAGE_WEBHOOKS")]
pub async fn unlink(
    ctx: Context<'_>,
    #[description = "Discord channel to unlink"]
    #[channel_types("Text")]
    channel: serenity::GuildChannel,
) -> Result<(), Error> {
    let guild_id = ctx
        .guild_id()
        .ok_or("this command can only be used in a guild")?
        .get();
    let removed = routes::unlink(
        &ctx.data().db,
        guild_id,
        channel.id.get(),
        Platform::Telegram,
    )
    .await?;
    if removed > 0 {
        ctx.say(format!("🗑️ Unlinked <#{}>.", channel.id.get()))
            .await?;
    } else {
        ctx.say("That channel isn't linked to a Telegram route.")
            .await?;
    }
    Ok(())
}

/// Show this server's proxy routes.
#[poise::command(slash_command, guild_only, required_permissions = "MANAGE_WEBHOOKS")]
pub async fn list(ctx: Context<'_>) -> Result<(), Error> {
    let guild_id = ctx
        .guild_id()
        .ok_or("this command can only be used in a guild")?
        .get();
    let routes = routes::list(&ctx.data().db, guild_id).await?;
    if routes.is_empty() {
        ctx.say("No proxy routes. Add one with `/proxy link`.")
            .await?;
        return Ok(());
    }
    let lines: Vec<String> = routes
        .iter()
        .map(|r| {
            format!(
                "• <#{}> ⇄ {} `{}`",
                r.discord_channel,
                r.platform.label(),
                r.remote_chat_id
            )
        })
        .collect();
    // Each remote_chat_id is stored free-form user input; suppress mentions on the echo.
    ctx.send(
        poise::CreateReply::default()
            .content(format!("**Proxy routes**\n{}", lines.join("\n")))
            .allowed_mentions(serenity::CreateAllowedMentions::new()),
    )
    .await?;
    Ok(())
}

/// Inject a synthetic inbound message to test the relay (bot owner only).
///
/// Stands in for the deferred Telegram client: it builds a [`RemoteMessage`] and runs it through
/// the real inbound path, so a webhook message appears in the channel linked to `remote_chat_id`.
#[poise::command(slash_command, guild_only, owners_only, rename = "proxytest")]
pub async fn proxy_test(
    ctx: Context<'_>,
    #[description = "Remote chat id of an already-linked route"] remote_chat_id: String,
    #[description = "Display name to relay as"] author: String,
    #[description = "Message text"] text: String,
    #[description = "Avatar URL (optional)"] avatar_url: Option<String>,
) -> Result<(), Error> {
    let msg = RemoteMessage {
        author,
        avatar_url,
        content: text,
    };
    match webhook::deliver_inbound(
        ctx.http(),
        &ctx.data().db,
        Platform::Telegram,
        remote_chat_id.trim(),
        &msg,
    )
    .await
    {
        Ok(()) => {
            ctx.send(
                poise::CreateReply::default()
                    .content("Injected synthetic inbound message.")
                    .ephemeral(true),
            )
            .await?;
            Ok(())
        }
        Err(e) => reply_or_propagate(ctx, e, true).await,
    }
}

/// The proxy commands, for [`crate::commands::all`].
pub fn commands() -> Vec<poise::Command<Data, Error>> {
    vec![proxy(), proxy_test()]
}
