mod commands;
mod config;
mod error;
mod events;

use poise::serenity_prelude as serenity;

/// Shared bot state handed to every command and event. Empty for the connection-only scaffold;
/// this is where a DB pool / HTTP (e.g. LLM) client / config handle will later live.
pub struct Data {}

/// Framework-wide error type. poise requires `std::error::Error` here, so this is the boxed-error
/// alias from poise's own examples (anyhow::Error does not implement `std::error::Error`).
pub type Error = Box<dyn std::error::Error + Send + Sync>;

/// Convenience alias for poise command contexts.
pub type Context<'a> = poise::Context<'a, Data, Error>;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Load .env if present (real DISCORD_TOKEN lives there, never committed).
    dotenvy::dotenv().ok();

    // Structured logging; honors RUST_LOG, defaults to `info`.
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
        )
        .init();

    let config = config::from_env()?;

    // non_privileged() is sufficient for slash commands. MESSAGE_CONTENT and GUILD_MEMBERS are
    // privileged intents — add them only when a feature (prefix commands, member events) needs them.
    let intents = serenity::GatewayIntents::non_privileged();

    let framework = poise::Framework::builder()
        .options(poise::FrameworkOptions {
            commands: commands::all(),
            event_handler: |ctx, event, framework, data| {
                Box::pin(events::event_handler(ctx, event, framework, data))
            },
            ..Default::default()
        })
        .setup(|ctx, _ready, framework| {
            Box::pin(async move {
                let commands = &framework.options().commands;
                // Guild-scoped registration (instant) when TEST_GUILD_ID is set; else global
                // (can take up to ~1h to propagate). Lets the dev loop iterate fast.
                match std::env::var("TEST_GUILD_ID")
                    .ok()
                    .and_then(|s| s.trim().parse::<u64>().ok())
                {
                    Some(id) => {
                        let guild = serenity::GuildId::new(id);
                        poise::builtins::register_in_guild(ctx, commands, guild).await?;
                        tracing::info!("registered {} commands in test guild {id}", commands.len());
                    }
                    None => {
                        poise::builtins::register_globally(ctx, commands).await?;
                        tracing::info!("registered {} commands globally", commands.len());
                    }
                }
                Ok(Data {})
            })
        })
        .build();

    tracing::info!("starting boyfucker…");
    let mut client = serenity::ClientBuilder::new(config.token, intents)
        .framework(framework)
        .await?;

    client.start().await?;
    Ok(())
}
