//! Integration with `herdr`, the terminal workspace manager for coding agents.
//! A task can own a herdr tab: launching one starts an agent in the task's
//! project, briefed with the task title and notes.

use anyhow::{anyhow, Result};
use serde_json::Value;
use std::collections::HashMap;
use std::process::Command;

/// Agent kinds offered in the launcher, in the order they're shown.
pub const KINDS: [&str; 6] = ["claude", "codex", "gemini", "opencode", "cursor", "amp"];

pub fn available() -> bool {
    Command::new("herdr")
        .arg("--version")
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

fn call(args: &[&str]) -> Result<Value> {
    let out = Command::new("herdr").args(args).output()?;
    let text = String::from_utf8_lossy(&out.stdout);
    let v: Value = serde_json::from_str(text.trim())
        .map_err(|_| anyhow!("{}", String::from_utf8_lossy(&out.stderr).trim()))?;
    if let Some(e) = v.get("error") {
        let msg = e
            .get("message")
            .and_then(|m| m.as_str())
            .unwrap_or("herdr error");
        return Err(anyhow!("{}", msg));
    }
    v.get("result")
        .cloned()
        .ok_or_else(|| anyhow!("no result in herdr response"))
}

/// herdr requires agent names of 1-32 chars, lowercase `[a-z0-9-_]`, starting
/// with a letter. Tab ids run t9, tA, tB… so they must be lowercased, not just
/// pasted in — an uppercase id is rejected as `invalid_agent_name`.
pub fn agent_name(tab_id: &str) -> String {
    format!("twodo-{}", tab_id)
        .to_lowercase()
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() { c } else { '-' })
        .take(32)
        .collect()
}

/// Creates a tab in `cwd`, starts `kind` in it, and submits the opening prompt.
/// Returns (tab_id, pane_id, agent_name).
pub fn launch(cwd: &str, label: &str, kind: &str, prompt: &str) -> Result<(String, String, String)> {
    let r = call(&["tab", "create", "--cwd", cwd, "--label", label, "--no-focus"])?;
    let tab_id = r
        .pointer("/tab/tab_id")
        .and_then(|x| x.as_str())
        .ok_or_else(|| anyhow!("herdr returned no tab_id"))?
        .to_string();
    let pane_id = r
        .pointer("/root_pane/pane_id")
        .and_then(|x| x.as_str())
        .ok_or_else(|| anyhow!("herdr returned no pane_id"))?
        .to_string();

    // Agent names must be unique within the session; the tab id already is.
    // herdr requires 1-32 chars of lowercase [a-z0-9-_] starting with a letter,
    // and its tab ids go t9, tA, tB… so the id must be lowercased, not just
    // pasted in.
    let name = agent_name(&tab_id);

    // A freshly created pane isn't at its shell prompt yet, so `agent start`
    // reports `agent_pane_busy` for the first second or so. Retry until it is.
    let mut last = anyhow!("agent never started");
    let mut started = false;
    for _ in 0..30 {
        match call(&["agent", "start", &name, "--kind", kind, "--pane", &pane_id]) {
            Ok(_) => {
                started = true;
                break;
            }
            Err(e) => {
                last = e;
                std::thread::sleep(std::time::Duration::from_millis(500));
            }
        }
    }
    if !started {
        // Don't leave an empty tab behind when the agent won't come up.
        let _ = close(&tab_id);
        return Err(last);
    }

    if !prompt.trim().is_empty() {
        // A failed prompt is not fatal — the agent is up and usable either way.
        let _ = call(&["agent", "prompt", &name, prompt]);
    }
    Ok((tab_id, pane_id, name))
}

pub fn focus(tab_id: &str) -> Result<()> {
    call(&["tab", "focus", tab_id]).map(|_| ())
}

pub fn close(tab_id: &str) -> Result<()> {
    call(&["tab", "close", tab_id]).map(|_| ())
}

/// tab_id -> agent status ("working", "idle", "unknown"). Tabs that no longer
/// exist are simply absent, which is how the UI detects a closed tab.
pub fn statuses() -> HashMap<String, String> {
    let mut out = HashMap::new();
    if let Ok(r) = call(&["tab", "list"]) {
        if let Some(tabs) = r.get("tabs").and_then(|t| t.as_array()) {
            for t in tabs {
                if let (Some(id), Some(st)) = (
                    t.get("tab_id").and_then(|x| x.as_str()),
                    t.get("agent_status").and_then(|x| x.as_str()),
                ) {
                    out.insert(id.to_string(), st.to_string());
                }
            }
        }
    }
    out
}
