//! PostgreSQL store for `proxy_routes` (I/O glue — compile + live-run verified).
//!
//! Mirrors `crate::access`: runtime `sqlx::query().bind(...)` (no `query!` macro, no build-time DB),
//! snowflakes stored via lossless `as i64` / read back via `as u64`.

use super::Platform;
use crate::Error;
use crate::error::ProxyError;
use sqlx::{PgPool, Row};

/// A proxy route as shown by `/proxy list`.
#[derive(Debug, Clone)]
pub struct ProxyRoute {
    pub discord_channel: u64,
    pub platform: Platform,
    pub remote_chat_id: String,
}

/// Link a Discord channel to a remote chat. Returns [`ProxyError::ChannelAlreadyLinked`] if the
/// channel is already linked for this platform *or* the remote chat is already linked elsewhere
/// (the `(platform, remote_chat_id)` unique index).
pub async fn link(
    pool: &PgPool,
    guild_id: u64,
    discord_channel: u64,
    platform: Platform,
    remote_chat_id: &str,
    created_by: u64,
) -> Result<(), Error> {
    let result = sqlx::query(
        "INSERT INTO proxy_routes (guild_id, discord_channel, platform, remote_chat_id, created_by) \
         VALUES ($1, $2, $3, $4, $5)",
    )
    .bind(guild_id as i64)
    .bind(discord_channel as i64)
    .bind(platform.as_i16())
    .bind(remote_chat_id)
    .bind(created_by as i64)
    .execute(pool)
    .await;

    match result {
        Ok(_) => Ok(()),
        // 23505 = unique_violation (PK on the channel, or the remote_chat_id index).
        Err(sqlx::Error::Database(db)) if db.code().as_deref() == Some("23505") => {
            Err(ProxyError::ChannelAlreadyLinked.into())
        }
        Err(e) => Err(e.into()),
    }
}

/// Unlink a channel for a platform. Returns the number of rows removed (0 if nothing was linked).
pub async fn unlink(
    pool: &PgPool,
    guild_id: u64,
    discord_channel: u64,
    platform: Platform,
) -> Result<u64, Error> {
    let result = sqlx::query(
        "DELETE FROM proxy_routes WHERE guild_id = $1 AND discord_channel = $2 AND platform = $3",
    )
    .bind(guild_id as i64)
    .bind(discord_channel as i64)
    .bind(platform.as_i16())
    .execute(pool)
    .await?;
    Ok(result.rows_affected())
}

/// List all routes in a guild (for `/proxy list`). Rows with an unknown platform discriminant are
/// skipped defensively rather than erroring the whole listing.
pub async fn list(pool: &PgPool, guild_id: u64) -> Result<Vec<ProxyRoute>, Error> {
    let rows = sqlx::query(
        "SELECT discord_channel, platform, remote_chat_id FROM proxy_routes WHERE guild_id = $1 \
         ORDER BY discord_channel",
    )
    .bind(guild_id as i64)
    .fetch_all(pool)
    .await?;
    let mut routes = Vec::with_capacity(rows.len());
    for row in rows {
        let channel: i64 = row.try_get("discord_channel")?;
        let platform_raw: i16 = row.try_get("platform")?;
        let remote_chat_id: String = row.try_get("remote_chat_id")?;
        if let Some(platform) = Platform::from_i16(platform_raw) {
            routes.push(ProxyRoute {
                discord_channel: channel as u64,
                platform,
                remote_chat_id,
            });
        }
    }
    Ok(routes)
}

/// Resolve the Discord channel id an inbound `(platform, remote_chat_id)` should be delivered to.
pub async fn fetch_by_remote(
    pool: &PgPool,
    platform: Platform,
    remote_chat_id: &str,
) -> Result<Option<u64>, Error> {
    let row = sqlx::query(
        "SELECT discord_channel FROM proxy_routes WHERE platform = $1 AND remote_chat_id = $2",
    )
    .bind(platform.as_i16())
    .bind(remote_chat_id)
    .fetch_optional(pool)
    .await?;
    match row {
        Some(row) => {
            let discord_channel: i64 = row.try_get("discord_channel")?;
            Ok(Some(discord_channel as u64))
        }
        None => Ok(None),
    }
}

/// Resolve the remote target a Discord `channel` is linked to (for the outbound direction).
/// Returns `None` if the channel is not a proxy channel.
pub async fn fetch_by_channel(
    pool: &PgPool,
    discord_channel: u64,
) -> Result<Option<(Platform, String)>, Error> {
    // A channel can in principle hold routes for multiple platforms (the PK allows it); pick the
    // lowest platform deterministically. ORDER BY makes the choice stable rather than arbitrary.
    let row = sqlx::query(
        "SELECT platform, remote_chat_id FROM proxy_routes WHERE discord_channel = $1 \
         ORDER BY platform LIMIT 1",
    )
    .bind(discord_channel as i64)
    .fetch_optional(pool)
    .await?;
    match row {
        Some(row) => {
            let platform_raw: i16 = row.try_get("platform")?;
            let remote_chat_id: String = row.try_get("remote_chat_id")?;
            match Platform::from_i16(platform_raw) {
                Some(platform) => Ok(Some((platform, remote_chat_id))),
                None => Ok(None),
            }
        }
        None => Ok(None),
    }
}
