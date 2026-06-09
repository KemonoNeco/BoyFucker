-- Per-guild moderation allowlist.
-- kind: 0 = user, 1 = role. Discord snowflakes are stored as BIGINT (lossless u64<->i64).
CREATE TABLE IF NOT EXISTS allowlist_entries (
    guild_id  BIGINT      NOT NULL,
    kind      SMALLINT    NOT NULL,
    entity_id BIGINT      NOT NULL,
    added_by  BIGINT      NOT NULL,
    added_at  TIMESTAMPTZ NOT NULL DEFAULT now(),
    PRIMARY KEY (guild_id, kind, entity_id)
);
