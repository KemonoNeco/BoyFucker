//! Per-guild moderation allowlist: the access gate, its PostgreSQL store, and the
//! `/allow`-family management commands.
//!
//! The only pure (unit-tested) piece is [`is_allowed`]. Everything else — the SQL and the
//! poise `check` — is I/O glue, verified by compile + live run.

use std::collections::HashSet;

/// Decide whether an invoker may use the moderation commands in a guild.
///
/// A user passes iff they are on the allowlist **by user ID** or **hold at least one
/// allowlisted role**. This is the whole gate — Manage-Server does NOT bypass it (it only
/// authorizes the management commands that edit these sets).
pub fn is_allowed(
    invoker_id: u64,
    invoker_role_ids: &[u64],
    allowed_user_ids: &HashSet<u64>,
    allowed_role_ids: &HashSet<u64>,
) -> bool {
    allowed_user_ids.contains(&invoker_id)
        || invoker_role_ids
            .iter()
            .any(|role_id| allowed_role_ids.contains(role_id))
}

// ---------------------------------------------------------------------------
// PostgreSQL store + poise wiring (I/O glue — compile + live-run verified).
// ---------------------------------------------------------------------------

use crate::{Context, Data, Error};
use poise::serenity_prelude as serenity;
use sqlx::{PgPool, Row};

const KIND_USER: i16 = 0;
const KIND_ROLE: i16 = 1;

/// A guild's allowlisted user and role IDs.
pub struct AllowedSets {
    pub users: HashSet<u64>,
    pub roles: HashSet<u64>,
}

/// Load a guild's allowlist from the database.
pub async fn fetch_allowed(pool: &PgPool, guild_id: u64) -> Result<AllowedSets, Error> {
    let rows = sqlx::query("SELECT kind, entity_id FROM allowlist_entries WHERE guild_id = $1")
        .bind(guild_id as i64)
        .fetch_all(pool)
        .await?;
    let mut users = HashSet::new();
    let mut roles = HashSet::new();
    for row in rows {
        let kind: i16 = row.try_get("kind")?;
        let entity_id: i64 = row.try_get("entity_id")?;
        match kind {
            KIND_USER => {
                users.insert(entity_id as u64);
            }
            KIND_ROLE => {
                roles.insert(entity_id as u64);
            }
            _ => {}
        }
    }
    Ok(AllowedSets { users, roles })
}

async fn add_entry(
    pool: &PgPool,
    guild_id: u64,
    kind: i16,
    entity_id: u64,
    added_by: u64,
) -> Result<(), Error> {
    sqlx::query(
        "INSERT INTO allowlist_entries (guild_id, kind, entity_id, added_by) \
         VALUES ($1, $2, $3, $4) ON CONFLICT (guild_id, kind, entity_id) DO NOTHING",
    )
    .bind(guild_id as i64)
    .bind(kind)
    .bind(entity_id as i64)
    .bind(added_by as i64)
    .execute(pool)
    .await?;
    Ok(())
}

async fn remove_entry(
    pool: &PgPool,
    guild_id: u64,
    kind: i16,
    entity_id: u64,
) -> Result<u64, Error> {
    let result = sqlx::query(
        "DELETE FROM allowlist_entries WHERE guild_id = $1 AND kind = $2 AND entity_id = $3",
    )
    .bind(guild_id as i64)
    .bind(kind)
    .bind(entity_id as i64)
    .execute(pool)
    .await?;
    Ok(result.rows_affected())
}

/// poise `check` for the moderation commands: pass iff the invoker is on the guild allowlist.
/// **Fails closed** — a DB error propagates and the command is refused, never allowed-on-error.
pub async fn moderation_access_check(ctx: Context<'_>) -> Result<bool, Error> {
    let Some(guild_id) = ctx.guild_id() else {
        ctx.say("This command can only be used in a server.")
            .await?;
        return Ok(false);
    };
    let invoker_id = ctx.author().id.get();
    // Roles come off the interaction member; collect to owned IDs before the DB await.
    let role_ids: Vec<u64> = match ctx.author_member().await {
        Some(member) => member.roles.iter().map(|r| r.get()).collect(),
        None => Vec::new(),
    };
    let allowed = fetch_allowed(&ctx.data().db, guild_id.get()).await?;
    if is_allowed(invoker_id, &role_ids, &allowed.users, &allowed.roles) {
        Ok(true)
    } else {
        ctx.say("⛔ You are not on this server's moderation allowlist. An admin can add you with `/allow`.")
            .await?;
        Ok(false)
    }
}

/// Add a user and/or role to this server's moderation allowlist (Manage Server only).
#[poise::command(slash_command, required_permissions = "MANAGE_GUILD", guild_only)]
pub async fn allow(
    ctx: Context<'_>,
    #[description = "User to allow"] user: Option<serenity::User>,
    #[description = "Role to allow"] role: Option<serenity::Role>,
) -> Result<(), Error> {
    let guild_id = ctx
        .guild_id()
        .ok_or("this command can only be used in a guild")?
        .get();
    let by = ctx.author().id.get();
    let pool = &ctx.data().db;
    let mut added = Vec::new();
    if let Some(u) = &user {
        add_entry(pool, guild_id, KIND_USER, u.id.get(), by).await?;
        added.push(format!("user **{}**", u.name));
    }
    if let Some(r) = &role {
        add_entry(pool, guild_id, KIND_ROLE, r.id.get(), by).await?;
        added.push(format!("role **{}**", r.name));
    }
    if added.is_empty() {
        ctx.say("Specify a user and/or a role to allow.").await?;
    } else {
        ctx.say(format!(
            "✅ Added to the moderation allowlist: {}.",
            added.join(", ")
        ))
        .await?;
    }
    Ok(())
}

/// Remove a user and/or role from this server's moderation allowlist (Manage Server only).
#[poise::command(slash_command, required_permissions = "MANAGE_GUILD", guild_only)]
pub async fn disallow(
    ctx: Context<'_>,
    #[description = "User to remove"] user: Option<serenity::User>,
    #[description = "Role to remove"] role: Option<serenity::Role>,
) -> Result<(), Error> {
    let guild_id = ctx
        .guild_id()
        .ok_or("this command can only be used in a guild")?
        .get();
    let pool = &ctx.data().db;
    let mut removed = Vec::new();
    if let Some(u) = &user
        && remove_entry(pool, guild_id, KIND_USER, u.id.get()).await? > 0
    {
        removed.push(format!("user **{}**", u.name));
    }
    if let Some(r) = &role
        && remove_entry(pool, guild_id, KIND_ROLE, r.id.get()).await? > 0
    {
        removed.push(format!("role **{}**", r.name));
    }
    if removed.is_empty() {
        ctx.say("Nothing removed — specify a user and/or role currently on the allowlist.")
            .await?;
    } else {
        ctx.say(format!(
            "🗑️ Removed from the moderation allowlist: {}.",
            removed.join(", ")
        ))
        .await?;
    }
    Ok(())
}

/// Show this server's moderation allowlist (Manage Server only).
// IDs are rendered in code spans so the listing never pings anyone.
#[poise::command(
    slash_command,
    required_permissions = "MANAGE_GUILD",
    guild_only,
    rename = "allowlist"
)]
pub async fn show_allowlist(ctx: Context<'_>) -> Result<(), Error> {
    let guild_id = ctx
        .guild_id()
        .ok_or("this command can only be used in a guild")?
        .get();
    let allowed = fetch_allowed(&ctx.data().db, guild_id).await?;
    if allowed.users.is_empty() && allowed.roles.is_empty() {
        ctx.say("The moderation allowlist is empty. Add entries with `/allow`.")
            .await?;
        return Ok(());
    }
    let fmt = |ids: &HashSet<u64>| {
        if ids.is_empty() {
            "—".to_string()
        } else {
            ids.iter()
                .map(|id| format!("`{id}`"))
                .collect::<Vec<_>>()
                .join(", ")
        }
    };
    ctx.say(format!(
        "**Moderation allowlist**\nUsers: {}\nRoles: {}",
        fmt(&allowed.users),
        fmt(&allowed.roles)
    ))
    .await?;
    Ok(())
}

/// The allowlist management commands, for [`crate::commands::all`].
pub fn commands() -> Vec<poise::Command<Data, Error>> {
    vec![allow(), disallow(), show_allowlist()]
}

#[cfg(test)]
mod tests {
    use super::*;

    fn set(ids: &[u64]) -> HashSet<u64> {
        ids.iter().copied().collect()
    }

    #[test]
    fn empty_allowlist_denies_everyone() {
        assert!(!is_allowed(7, &[1, 2, 3], &set(&[]), &set(&[])));
    }

    #[test]
    fn user_on_user_allowlist_is_allowed() {
        assert!(is_allowed(7, &[], &set(&[7]), &set(&[])));
    }

    #[test]
    fn user_not_listed_and_no_roles_is_denied() {
        assert!(!is_allowed(7, &[], &set(&[1, 2]), &set(&[9])));
    }

    #[test]
    fn user_holding_an_allowed_role_is_allowed() {
        // Not on the user list, but role 3 is allowlisted and the invoker holds it.
        assert!(is_allowed(7, &[1, 2, 3], &set(&[]), &set(&[3])));
    }

    #[test]
    fn user_holding_only_non_allowed_roles_is_denied() {
        assert!(!is_allowed(7, &[1, 2], &set(&[]), &set(&[3, 4])));
    }

    #[test]
    fn role_intersection_anywhere_in_the_list_is_allowed() {
        // Overlap on the last role only — `any` must scan the whole list.
        assert!(is_allowed(7, &[10, 11, 12], &set(&[]), &set(&[12])));
    }

    #[test]
    fn user_match_takes_precedence_even_with_no_role_overlap() {
        assert!(is_allowed(7, &[1], &set(&[7]), &set(&[99])));
    }

    #[test]
    fn both_user_and_role_match_is_allowed() {
        assert!(is_allowed(7, &[3], &set(&[7]), &set(&[3])));
    }
}
