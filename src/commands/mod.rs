pub mod moderation;
pub mod poll;
pub mod proxy;
pub mod voice;

use crate::{Data, Error};

/// All registered bot commands. Registered on startup (guild-scoped if `TEST_GUILD_ID` is set,
/// otherwise globally — see the `setup` closure in `main`).
pub fn all() -> Vec<poise::Command<Data, Error>> {
    let mut cmds = moderation::commands();
    cmds.extend(crate::access::commands());
    cmds.extend(poll::commands());
    cmds.extend(voice::commands());
    cmds.extend(proxy::commands());
    cmds
}
