//! Everything that is not UI: GitHub, installs, resource preparation, the game's processes and
//! updates. The GUI, the CLI and the MCP server all go through this module. Functions here
//! block; the GUI runs them on threads and reads their messages.
use crate::{account, mojang, sources};
use std::fs;
use std::io::{BufRead, BufReader, Read, Write};
use std::path::{Path, PathBuf};
use std::process::{Child, ChildStdin, Command, Stdio};
use std::sync::mpsc::Sender;
use std::sync::{Arc, Mutex};

pub const REPO: &str = "horizon-rust/horizon";
pub const README_URL: &str = "https://github.com/horizon-rust/horizon/blob/main/launcher/README.md";
/// The players' page on connecting an AI agent (the website's docs/agents.md).
pub const AGENT_DOCS_URL: &str = "https://playhorizon.dev/docs/agents";
/// This launcher's version: the release workflow sets `HORIZON_LAUNCHER_VERSION` from the
/// tag it builds (no version-bump commits); other builds say Cargo.toml's.
pub const VERSION: &str = version_or(option_env!("HORIZON_LAUNCHER_VERSION"));
/// Launcher releases are tagged `launcher@X.Y.Z` from 0.2 on (docs/versioning.md), and Horizon's
/// own `vX.Y.Z`. Before 0.2 they were `launcher-vX.Y.Z` and `vYYYY.MM.DD[.N]`: every 0.2 tag is
/// newer than every one of those ([`release_order`]).
pub const LAUNCHER_TAG_PREFIX: &str = "launcher@";
/// The launcher tags from before 0.2 (`launcher-v0.16.0`).
pub const OLD_LAUNCHER_TAG_PREFIX: &str = "launcher-v";

const fn version_or(injected: Option<&'static str>) -> &'static str {
    match injected {
        Some(v) if !v.is_empty() => v,
        _ => env!("CARGO_PKG_VERSION"),
    }
}

#[derive(Clone, Debug, serde::Serialize)]
pub struct Release {
    pub tag: String,
    pub name: String,
    pub body: String,
    pub date: String,
    pub asset: Option<(String, u64)>,
    /// Listed and installed by hand, never as an update: a GitHub pre-release of the old Alpha
    /// channel (v0.2.1 on), or a Nightly older than the current one.
    pub prerelease: bool,
    /// A Nightly's GitHub tag (`nightly`, or `nightly-<date>` for the last week's): its files and
    /// signature are there, while `tag` is the version it installs as (`nightly-2026-10-28-0c02067`).
    /// None for a release.
    pub nightly: Option<String>,
}

#[cfg(all(target_os = "linux", target_arch = "x86_64"))]
pub const ASSET: Option<&str> = Some("horizon-linux-x86_64.tar.gz");
#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
pub const ASSET: Option<&str> = Some("horizon-macos-arm64.zip");
#[cfg(all(target_os = "windows", target_arch = "x86_64"))]
pub const ASSET: Option<&str> = Some("horizon-windows-x64.zip");
#[cfg(not(any(
    all(target_os = "linux", target_arch = "x86_64"),
    all(target_os = "macos", target_arch = "aarch64"),
    all(target_os = "windows", target_arch = "x86_64")
)))]
pub const ASSET: Option<&str> = None;

#[cfg(all(target_os = "linux", target_arch = "x86_64"))]
pub const LAUNCHER_ASSET: Option<&str> = Some("horizon-launcher-linux-x86_64.tar.gz");
#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
pub const LAUNCHER_ASSET: Option<&str> = Some("horizon-launcher-macos-arm64.zip");
#[cfg(all(target_os = "windows", target_arch = "x86_64"))]
pub const LAUNCHER_ASSET: Option<&str> = Some("horizon-launcher-windows-x64.zip");
#[cfg(not(any(
    all(target_os = "linux", target_arch = "x86_64"),
    all(target_os = "macos", target_arch = "aarch64"),
    all(target_os = "windows", target_arch = "x86_64")
)))]
pub const LAUNCHER_ASSET: Option<&str> = None;

const LAUNCHER_BINARY: &str = if cfg!(windows) {
    "horizon-launcher.exe"
} else {
    "horizon-launcher"
};

pub(crate) const CLIENT: &str = if cfg!(windows) {
    "horizon-client.exe"
} else {
    "horizon-client"
};

/// The agent `url` is fetched with: the launcher's own user agent, through the player's proxy when
/// one is set (`proxy`).
fn agent(url: &str) -> ureq::Agent {
    crate::proxy::agent(
        ureq::AgentBuilder::new().user_agent(&format!("horizon-launcher/{VERSION}")),
        url,
    )
    .build()
}
