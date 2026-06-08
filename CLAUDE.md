# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## What this is

`boyfucker` — a personal Discord bot (Rust). Currently a **connection-only scaffold**: it authenticates, connects to the gateway, and logs "Logged in as …" but registers **no commands yet**. The first planned feature is basic moderation; an LLM integration may follow.

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

**Running locally:** copy `.env.example` → `.env` and fill in `DISCORD_TOKEN` (gitignored — never commit it). With no token, `cargo run` exits with `Error: DISCORD_TOKEN environment variable is not set` — that's the expected config-failure path, not a bug. `RUST_LOG` controls log level (default `info`), e.g. `RUST_LOG=boyfucker=debug,serenity=warn`.

## Architecture

Built on **poise 0.6** (command framework) over **serenity 0.12** (Discord API). poise re-exports serenity as `poise::serenity_prelude`; **depend on poise, not serenity directly**, so the serenity version is always whatever poise pins — this avoids version-skew breakage.

Three framework-wide types are defined in `main.rs` (crate root) and shared by submodules as `crate::{Data, Error, Context}`:
- `Data` — per-bot shared state, currently empty. **This is where a DB pool / HTTP (LLM) client / config handle goes** when added; it's threaded into every command and event.
- `Error = Box<dyn std::error::Error + Send + Sync>` — poise's framework error type. Note: poise requires `std::error::Error` here, which `anyhow::Error` does **not** implement, so do not swap this for `anyhow::Error`. (`main()` itself returns `anyhow::Result` for startup; that's separate.)

Module map:
- `main.rs` — entry point. `dotenvy` → tracing init → `config::from_env()` → build poise `Framework` (commands + event handler + `register_globally` in setup) → start the serenity client. Uses `GatewayIntents::non_privileged()`.
- `config.rs` — `Config::from_token(Option<String>)` is the **pure, fully-unit-tested** validation; `from_env()` is a thin I/O wrapper that reads `DISCORD_TOKEN` and delegates. Add new config there.
- `error.rs` — the single `BotError` enum (thiserror). Its Display messages mention `DISCORD_TOKEN` so a developer sees what to fix.
- `commands/mod.rs` — `all() -> Vec<Command<Data, Error>>` (empty; commands registered globally on startup). Add commands here; a commented `ping` shows the shape.
- `events/mod.rs` — `event_handler` matches `serenity::FullEvent`; currently only logs `Ready`.

## Conventions that aren't obvious from the code

- **Test seam discipline.** Validation lives in pure functions (`Config::from_token(Option<String>)`) that take their input as arguments; the env-reading wrapper (`from_env`) is left untested. **Do not write tests that call `std::env::set_var`/`remove_var`** — on edition 2024 these are `unsafe` and a data race across parallel tests. Test the pure function instead.
- **One `BotError`.** All error variants live in `error.rs`. Don't define module-local error enums — return `crate::error::BotError` (or a wrapper) so error messages stay consistent and reachable on the live path.
- **Intents are minimal on purpose.** `non_privileged()` is enough for slash commands. `MESSAGE_CONTENT`, `GUILD_MEMBERS`, presence, etc. are privileged — add one only when a feature needs it (e.g. moderation member events), and update the bot's invite scopes accordingly.
- **I/O glue (`main.rs`, the framework builder, event handlers) has no unit-test seam** — verify it by `cargo run` / live behavior, not tests. Logic worth testing should be factored into a pure function first.

## Git

Default branch is `main`. Private repo: `github.com/KemonoNeco/BoyFucker`.
