-- Per-channel proxy routes: maps a Discord channel to a remote chat on an external platform.
-- platform: 0 = telegram (extensible). Discord snowflakes are BIGINT (lossless u64<->i64).
-- remote_chat_id is TEXT — remote ids are signed/large and vary by platform (Telegram chat ids
-- can be negative for groups). The managed webhook is discovered/created on demand and held in
-- memory, so no webhook token is persisted (no secret at rest).
CREATE TABLE IF NOT EXISTS proxy_routes (
    guild_id        BIGINT      NOT NULL,
    discord_channel BIGINT      NOT NULL,
    platform        SMALLINT    NOT NULL,
    remote_chat_id  TEXT        NOT NULL,
    created_by      BIGINT      NOT NULL,
    created_at      TIMESTAMPTZ NOT NULL DEFAULT now(),
    PRIMARY KEY (guild_id, discord_channel, platform)
);

-- Reverse (inbound) lookup: a given remote chat maps to exactly one Discord channel.
CREATE UNIQUE INDEX IF NOT EXISTS proxy_routes_remote
    ON proxy_routes (platform, remote_chat_id);
