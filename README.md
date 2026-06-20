# BoyFucker

A personal Discord moderation bot written in Rust on
[poise](https://github.com/serenity-rs/poise) + [serenity](https://github.com/serenity-rs/serenity).

## Commands

Slash commands, each gated by the matching Discord permission:

| Command | Permission | Description |
|---|---|---|
| `/purge <count>` | Manage Messages | Bulk-delete 1–100 recent messages |
| `/kick <member> [reason]` | Kick Members | Kick a member |
| `/ban <member> [delete_message_days] [reason]` | Ban Members | Ban a member, optionally deleting 0–7 days of their messages |
| `/unban <user>` | Ban Members | Unban a user by ID |
| `/mute <member> <duration>` | Moderate Members | Timeout a member (`30s`, `10m`, `2h`, `7d`; max 28 days) |
| `/unmute <member>` | Moderate Members | Clear a member's timeout |

Moderation actions also refuse to target yourself, the bot, or the guild owner, and —
unless you are the owner — require your top role to be strictly higher than the target's.

Utility commands (open to everyone — no permission or allowlist gate):

| Command | Description |
|---|---|
| `/poll <question> <options> [duration] [multiple]` | Create a native Discord poll. Options are separated by `\|` (2–10), e.g. `Pizza \| Sushi \| Tacos`. `duration` accepts `6h`, `2d`, … (1 hour to 32 days; default 24h); `multiple` lets voters pick more than one option. Discord runs the vote and tallies results. |
| `/join [channel]` | Have the bot join a voice channel and sit there (presence only, no audio — keeps the channel active). With no `channel`, joins the voice channel you're currently in. Groundwork for future voice features. |
| `/leave` | Disconnect the bot from the voice channel it's in. |

## Setup

1. Create an application at the [Discord Developer Portal](https://discord.com/developers/applications),
   add a bot, copy its token, and invite it to your server with the moderation permissions you want
   it to have.
2. Copy `.env.example` to `.env` and fill it in:

   ```
   DISCORD_TOKEN=your-bot-token
   # optional: register commands instantly in one guild while developing
   TEST_GUILD_ID=your-test-guild-id
   ```

   `.env` is gitignored — never commit your token.
3. Run it:

   ```
   cargo run
   ```

With `TEST_GUILD_ID` set, slash commands register in that guild instantly; without it they register
globally (which can take up to ~1 hour to propagate). `RUST_LOG` controls log level (default `info`).

## Development

```
cargo test                  # unit tests
cargo clippy --all-targets  # lint
cargo fmt                   # format
```

The pure validation/authorization logic — purge-count bounds, timeout-duration parsing, ban-day
range, and the role-hierarchy precedence — lives in `src/commands/moderation.rs` and is unit-tested.
The `/poll` input validators (question/options/duration) live in `src/commands/poll.rs`, also pure
and unit-tested. `/join`'s channel-resolution logic (`resolve_join_target`) lives in
`src/commands/voice.rs`, likewise pure and unit-tested; its handler asks
[songbird](https://github.com/serenity-rs/songbird) (gateway-only, no audio driver) to join. The
command handlers are thin wiring over serenity's HTTP API.
