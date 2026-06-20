//! Message proxying between Discord and external chat platforms (Telegram, …).
//!
//! This PR ships the **Discord-side** of a bidirectional bridge. The external platform clients are
//! deferred behind two seams:
//! - **Inbound** (remote → Discord): a future adapter calls [`webhook::deliver_inbound`] with a
//!   [`RemoteMessage`]; we relay it into the mapped channel via a per-channel webhook so each
//!   remote sender appears as themselves.
//! - **Outbound** (Discord → remote): the message event handler builds a [`RelayMessage`] and
//!   hands it to an [`Egress`] sink. This PR provides only [`LoggingEgress`]; the Telegram adapter
//!   will implement a real one.
//!
//! The pure, unit-tested logic lives in [`sanitize`], [`username`], and [`transform`]; the SQL
//! ([`routes`]) and serenity HTTP ([`webhook`]) are glue, verified live.

pub mod routes;
pub mod sanitize;
pub mod transform;
pub mod username;
pub mod webhook;

use crate::Error;
use std::future::Future;
use std::pin::Pin;

/// External platforms a Discord channel can be bridged to. Stored as `SMALLINT` in `proxy_routes`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Platform {
    Telegram,
}

impl Platform {
    /// The on-disk discriminant (kept stable — it is persisted).
    pub fn as_i16(self) -> i16 {
        match self {
            Platform::Telegram => 0,
        }
    }

    /// Parse a stored discriminant back to a [`Platform`]; unknown values yield `None`.
    pub fn from_i16(v: i16) -> Option<Platform> {
        match v {
            0 => Some(Platform::Telegram),
            _ => None,
        }
    }

    /// Human label for command replies / logs.
    pub fn label(self) -> &'static str {
        match self {
            Platform::Telegram => "Telegram",
        }
    }
}

/// An inbound message arriving from a remote platform, to be relayed into Discord (the inbound
/// seam's DTO). `avatar_url`, when present, is shown as the webhook message's avatar.
#[derive(Debug, Clone)]
pub struct RemoteMessage {
    pub author: String,
    pub avatar_url: Option<String>,
    pub content: String,
}

/// An outbound message leaving Discord for a remote platform, handed to an [`Egress`].
#[derive(Debug, Clone)]
pub struct RelayMessage {
    pub platform: Platform,
    pub remote_chat_id: String,
    pub author: String,
    pub content: String,
}

/// Sink for outbound (Discord → remote) messages. The seam a real platform client plugs into.
///
/// Defined with a manually-boxed future (rather than `async fn`) so it is object-safe and can be
/// stored as `Arc<dyn Egress>` on [`crate::Data`] — no `async_trait` dependency needed.
pub trait Egress: Send + Sync {
    fn send<'a>(
        &'a self,
        msg: RelayMessage,
    ) -> Pin<Box<dyn Future<Output = Result<(), Error>> + Send + 'a>>;
}

/// The only [`Egress`] this PR ships: log what *would* be sent. Replaced by the Telegram adapter.
pub struct LoggingEgress;

impl Egress for LoggingEgress {
    fn send<'a>(
        &'a self,
        msg: RelayMessage,
    ) -> Pin<Box<dyn Future<Output = Result<(), Error>> + Send + 'a>> {
        Box::pin(async move {
            tracing::info!(
                platform = msg.platform.label(),
                remote_chat_id = %msg.remote_chat_id,
                "outbound relay: {}",
                transform::format_outbound(&msg.author, &msg.content)
            );
            Ok(())
        })
    }
}
