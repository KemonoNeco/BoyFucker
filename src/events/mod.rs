use crate::{Data, Error};
use poise::serenity_prelude as serenity;

/// Gateway event hook. Currently only logs the `Ready` event (the connect-success signal);
/// add further `FullEvent` arms here as the bot grows.
pub async fn event_handler(
    _ctx: &serenity::Context,
    event: &serenity::FullEvent,
    _framework: poise::FrameworkContext<'_, Data, Error>,
    _data: &Data,
) -> Result<(), Error> {
    if let serenity::FullEvent::Ready { data_about_bot, .. } = event {
        tracing::info!("Logged in as {}", data_about_bot.user.name);
    }
    Ok(())
}
