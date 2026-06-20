//! Inbound relay (remote → Discord) via a per-channel webhook (I/O glue — live-verified).
//!
//! Each proxied sender is shown as themselves by overriding the webhook execute's `username` +
//! `avatar_url`. The managed webhook is discovered/created on demand and not persisted (the
//! serenity `Webhook` carries its token internally, so its token never touches the database).
//! [`deliver_inbound`] is the inbound seam a future Telegram adapter calls.

use super::sanitize::prepare_relay_content;
use super::username::derive_webhook_username;
use super::{Platform, RemoteMessage, routes};
use crate::Error;
use crate::error::ProxyError;
use poise::serenity_prelude as serenity;
use sqlx::PgPool;

/// Name of the webhook this bot manages in each proxied channel.
const MANAGED_WEBHOOK_NAME: &str = "boyfucker-proxy";

/// Discord's per-channel webhook cap; creating beyond it fails, so we surface it as a clear error.
const MAX_CHANNEL_WEBHOOKS: usize = 15;

/// Find this bot's managed webhook in `channel`, creating it if absent.
async fn get_or_create_webhook(
    http: &serenity::Http,
    channel: serenity::ChannelId,
) -> Result<serenity::Webhook, Error> {
    let hooks = channel.webhooks(http).await?;
    if let Some(existing) = hooks
        .iter()
        .find(|h| h.name.as_deref() == Some(MANAGED_WEBHOOK_NAME) && h.token.is_some())
    {
        return Ok(existing.clone());
    }
    if hooks.len() >= MAX_CHANNEL_WEBHOOKS {
        return Err(ProxyError::WebhookLimitReached.into());
    }
    // Created without an avatar — the per-sender face is set per execute via `avatar_url`.
    let created = channel
        .create_webhook(http, serenity::CreateWebhook::new(MANAGED_WEBHOOK_NAME))
        .await?;
    Ok(created)
}

/// Relay one remote message into a specific Discord `channel` as its original author.
///
/// Pings are neutralized two ways: empty `allowed_mentions` (the authoritative API guard) and the
/// pure [`sanitize_content`] text transform; the username is made Discord-legal by
/// [`derive_webhook_username`].
pub async fn relay_inbound(
    http: &serenity::Http,
    channel: serenity::ChannelId,
    msg: &RemoteMessage,
) -> Result<(), Error> {
    // Sanitize + clamp to Discord's 2000-char limit up front; if nothing's left to send (empty or
    // media-only), skip silently rather than creating a webhook and 400-ing on an empty message.
    let Some(content) = prepare_relay_content(&msg.content) else {
        return Ok(());
    };

    let webhook = get_or_create_webhook(http, channel).await?;

    let mut builder = serenity::ExecuteWebhook::new()
        .content(content)
        .username(derive_webhook_username(&msg.author))
        // Empty allowed-mentions = mention nothing; must be set explicitly (serenity otherwise
        // falls back to the http default).
        .allowed_mentions(serenity::CreateAllowedMentions::new());
    if let Some(avatar) = msg.avatar_url.as_deref() {
        builder = builder.avatar_url(avatar);
    }

    // wait = false: fire-and-forget, we don't need the resulting message back.
    webhook.execute(http, false, builder).await?;
    Ok(())
}

/// The inbound seam: resolve `(platform, remote_chat_id)` to its linked channel and relay.
///
/// Returns [`ProxyError::NoRouteForRemote`] if no channel is linked to that remote chat.
pub async fn deliver_inbound(
    http: &serenity::Http,
    db: &PgPool,
    platform: Platform,
    remote_chat_id: &str,
    msg: &RemoteMessage,
) -> Result<(), Error> {
    let channel = routes::fetch_by_remote(db, platform, remote_chat_id)
        .await?
        .ok_or(ProxyError::NoRouteForRemote)?;
    relay_inbound(http, serenity::ChannelId::new(channel), msg).await
}
