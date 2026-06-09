# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## What this is

`boyfucker` — a personal Discord moderation bot (Rust). It connects to the gateway, logs "Logged in as …", and serves six moderation slash commands: `/purge`, `/kick`, `/ban`, `/unban`, `/mute` (timeout), `/unmute`. An LLM integration may follow.

Names: crate `boyfucker`, GitHub repo `BoyFucker`, Discord display name `Boyfucker` (the display name is set in the Developer Portal, not in code, so treat it as the source of truth over anything hardcoded).

## Commands

```bash
cargo build                 # build (first build is slow — serenity/tokio tree)
cargo test                  # run all unit tests
cargo test from_token       # run a single test / filter by substring
cargo clippy --all-targets  # lint — keep this clean (zero warnings is the bar)
cargo fmt                    # format
cargo run                    # run the bot (reads DISCORD_TOKEN; see below)
cargo watch -x run           # hot-reload dev loop (needs: cargo install cargo-watch)
```

**Running locally:** copy `.env.example` → `.env` and fill in `DISCORD_TOKEN` (gitignored — never commit it). With no token, `cargo run` exits with `Error: DISCORD_TOKEN environment variable is not set` — that's the expected config-failure path, not a bug. Set `TEST_GUILD_ID` to register slash commands in one guild **instantly** during dev (without it they register globally, ~1h propagation). `RUST_LOG` controls log level (default `info`), e.g. `RUST_LOG=boyfucker=debug,serenity=warn`.

## Architecture

Built on **poise 0.6** (command framework) over **serenity 0.12** (Discord API). poise re-exports serenity as `poise::serenity_prelude`; **depend on poise, not serenity directly**, so the serenity version is always whatever poise pins — this avoids version-skew breakage.

Three framework-wide types are defined in `main.rs` (crate root) and shared by submodules as `crate::{Data, Error, Context}`:
- `Data` — per-bot shared state, currently empty. **This is where a DB pool / HTTP (LLM) client / config handle goes** when added; it's threaded into every command and event.
- `Error = Box<dyn std::error::Error + Send + Sync>` — poise's framework error type. Note: poise requires `std::error::Error` here, which `anyhow::Error` does **not** implement, so do not swap this for `anyhow::Error`. (`main()` itself returns `anyhow::Result` for startup; that's separate.)

Module map:
- `main.rs` — entry point. `dotenvy` → tracing init → `config::from_env()` → build poise `Framework` → register commands in setup (`register_in_guild` when `TEST_GUILD_ID` is set, else `register_globally`) → start the serenity client. Uses `GatewayIntents::non_privileged()`.
- `config.rs` — `Config::from_token(Option<String>)` is the **pure, fully-unit-tested** validation; `from_env()` is a thin I/O wrapper that reads `DISCORD_TOKEN` and delegates. Add new config there.
- `error.rs` — the project's error enums (thiserror): `BotError` (startup) and `ModError` (moderation, user-facing Display strings). Error types live here; consumers `use crate::error::…`.
- `commands/mod.rs` — `all()` returns `moderation::commands()`. `commands/moderation.rs` holds **both** the pure, unit-tested validators (`validate_purge_count`, `validate_ban_delete_days`, `parse_timeout_duration`, `check_moderation_allowed`) **and** the six slash-command handlers (thin glue over serenity HTTP) plus `authorize()` (builds a `ModCheck` from live ctx + cached role positions).
- `events/mod.rs` — `event_handler` matches `serenity::FullEvent`; currently only logs `Ready`.

## Conventions that aren't obvious from the code

- **Test seam discipline.** Validation lives in pure functions (`Config::from_token(Option<String>)`) that take their input as arguments; the env-reading wrapper (`from_env`) is left untested. **Do not write tests that call `std::env::set_var`/`remove_var`** — on edition 2024 these are `unsafe` and a data race across parallel tests. Test the pure function instead.
- **Error enums live in `error.rs`; import them, don't redefine.** Both `BotError` and `ModError` live in `error.rs`; consumers `use crate::error::…`. Do **not** define a module-local error enum that duplicates one — that produces a dead canonical type while the live path carries different (often wrong) messages. (This collision happened twice during development when a type in `error.rs` had consumers in another module; the discriminator is whether the *Display string on the live error path* matches the contract.)
- **Validators are pure + arg-driven; handlers are glue.** New moderation logic worth testing goes in a pure function in `moderation.rs` (takes resolved facts as arguments, returns `Result<_, ModError>`), unit-tested and mutation-checked. The slash-command handler then resolves live data, calls the validator, and performs the serenity action. In `authorize()`, never hold the `ctx.guild()` cache ref across an `.await` (it isn't `Send`) — extract role positions in a sync block first.
- **Intents are minimal on purpose.** `non_privileged()` is enough for slash commands. `MESSAGE_CONTENT`, `GUILD_MEMBERS`, presence, etc. are privileged — add one only when a feature needs it (e.g. moderation member events), and update the bot's invite scopes accordingly.
- **I/O glue (`main.rs`, the framework builder, event handlers) has no unit-test seam** — verify it by `cargo run` / live behavior, not tests. Logic worth testing should be factored into a pure function first.

## Git

Default branch is `main`. Repo: `github.com/KemonoNeco/BoyFucker`.
