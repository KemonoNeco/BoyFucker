use crate::{Data, Error};

/// All registered bot commands. Empty for the connection-only scaffold — add commands here and
/// they are registered globally on startup (see `register_globally` in `main`).
///
/// First planned feature is a `moderation` submodule (kick / ban / timeout / purge). Those act via
/// serenity's HTTP API, so they need matching invite permissions (and possibly the privileged
/// `GUILD_MEMBERS` intent) but no extra gateway intents wired here.
pub fn all() -> Vec<poise::Command<Data, Error>> {
    vec![
        // ping(),  // uncomment to register the example command below
    ]
}

// Example slash command — the shape every future command follows. Uncomment + add `ping()` to the
// vec above to register it.
//
// #[poise::command(slash_command)]
// async fn ping(ctx: crate::Context<'_>) -> Result<(), Error> {
//     ctx.say("pong!").await?;
//     Ok(())
// }
