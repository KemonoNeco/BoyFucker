use crate::proxy::{RelayMessage, routes, transform};
use crate::{Data, Error};
use poise::serenity_prelude as serenity;

/// Gateway event hook. Logs `Ready`, and relays messages in proxy-linked channels outbound to
/// their remote platform (via the [`crate::proxy::Egress`] on [`Data`]).
pub async fn event_handler(
    ctx: &serenity::Context,
    event: &serenity::FullEvent,
    _framework: poise::FrameworkContext<'_, Data, Error>,
    data: &Data,
) -> Result<(), Error> {
    match event {
        serenity::FullEvent::Ready { data_about_bot, .. } => {
            tracing::info!("Logged in as {}", data_about_bot.user.name);
        }
        serenity::FullEvent::Message { new_message } => {
            handle_outbound(ctx, data, new_message).await?;
        }
        _ => {}
    }
    Ok(())
}

/// Relay a Discord message outbound to its linked remote chat, if it qualifies.
///
/// The loop guard ([`transform::should_relay`]) skips our own webhook echoes, the bot's own
/// messages, and empty content; non-linked channels are ignored after the cheap gate.
async fn handle_outbound(
    ctx: &serenity::Context,
    data: &Data,
    msg: &serenity::Message,
) -> Result<(), Error> {
    // Bot id from the cache — the borrow is a temporary, never held across an `.await`.
    let bot_id = ctx.cache.current_user().id.get();
    let webhook_id = msg.webhook_id.map(serenity::WebhookId::get);

    if !transform::should_relay(webhook_id, msg.author.id.get(), bot_id, &msg.content) {
        return Ok(());
    }

    // Only channels with a proxy route relay onward.
    let Some((platform, remote_chat_id)) =
        routes::fetch_by_channel(&data.db, msg.channel_id.get()).await?
    else {
        return Ok(());
    };

    let author = msg
        .author
        .global_name
        .clone()
        .unwrap_or_else(|| msg.author.name.clone());

    data.egress
        .send(RelayMessage {
            platform,
            remote_chat_id,
            author,
            content: msg.content.clone(),
        })
        .await?;
    Ok(())
}
