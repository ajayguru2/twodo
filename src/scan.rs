//! Reads Claude Code and Codex session logs and extracts per-agent activity,
//! so tasks can be credited with real agent usage without any manual tagging.

use anyhow::Result;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;
use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct AgentRun {
    /// "claude" or "codex"
    pub source: String,
    pub session_id: String,
    /// "main" for the top-level agent, otherwise the subagent type.
    pub kind: String,
    pub is_sub: bool,
    pub cwd: String,
    pub start: DateTime<Utc>,
    pub end: DateTime<Utc>,
    pub tool_calls: u32,
    pub input_tokens: u64,
    pub output_tokens: u64,
    /// Log file this run was read from, so the UI can point you at it.
    #[serde(default)]
    pub file: String,
}

#[derive(Serialize, Deserialize, Default)]
struct Cache {
    /// path -> (mtime_secs, len, runs)
    files: HashMap<String, (u64, u64, Vec<AgentRun>)>,
}

fn cache_path() -> PathBuf {
    crate::model::data_dir().join("scan-cache.json")
}

fn ts(v: &Value, key: &str) -> Option<DateTime<Utc>> {
    v.get(key)?
        .as_str()?
        .parse::<DateTime<Utc>>()
        .ok()
        .map(|d| d.with_timezone(&Utc))
}

fn file_stamp(p: &Path) -> (u64, u64) {
    match std::fs::metadata(p) {
        Ok(m) => {
            let mt = m
                .modified()
                .ok()
                .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
                .map(|d| d.as_secs())
                .unwrap_or(0);
            (mt, m.len())
        }
        Err(_) => (0, 0),
    }
}

/// Scans both providers. `since` prunes files untouched before the earliest
/// task window, which is what keeps this fast on large histories.
pub fn scan_all(since: Option<DateTime<Utc>>) -> Result<Vec<AgentRun>> {
    let mut cache: Cache = std::fs::read(cache_path())
        .ok()
        .and_then(|b| serde_json::from_slice(&b).ok())
        .unwrap_or_default();

    let cutoff = since.map(|s| s.timestamp() as u64).unwrap_or(0);
    let mut runs = Vec::new();
    let mut seen: Vec<String> = Vec::new();

    let home = dirs::home_dir().unwrap_or_default();
    let mut files: Vec<(PathBuf, &'static str)> = Vec::new();
    for (root, src) in [
        (home.join(".claude/projects"), "claude"),
        (home.join(".codex/sessions"), "codex"),
    ] {
        if !root.exists() {
            continue;
        }
        for e in walkdir::WalkDir::new(&root)
            .max_depth(6)
            .into_iter()
            .filter_map(|e| e.ok())
        {
            if e.file_type().is_file() && e.path().extension().is_some_and(|x| x == "jsonl") {
                files.push((e.path().to_path_buf(), src));
            }
        }
    }

    for (path, src) in files {
        let key = path.to_string_lossy().to_string();
        let (mtime, len) = file_stamp(&path);
        // A session that ended before every task window began cannot match.
        if mtime < cutoff {
            continue;
        }
        seen.push(key.clone());
        if let Some((cm, cl, cached)) = cache.files.get(&key) {
            if *cm == mtime && *cl == len {
                runs.extend(cached.iter().cloned());
                continue;
            }
        }
        let parsed = match src {
            "claude" => parse_claude(&path).unwrap_or_default(),
            _ => parse_codex(&path).unwrap_or_default(),
        };
        let mut parsed = parsed;
        for r in &mut parsed {
            r.file = key.clone();
        }
        cache.files.insert(key, (mtime, len, parsed.clone()));
        runs.extend(parsed);
    }

    cache.files.retain(|k, _| seen.contains(k));
    let _ = std::fs::create_dir_all(crate::model::data_dir());
    if let Ok(b) = serde_json::to_vec(&cache) {
        let _ = std::fs::write(cache_path(), b);
    }
    Ok(runs)
}

/// Claude Code: one JSONL per session. The session itself is the main agent;
/// every `Task`/`Agent` tool_use is a subagent spawn.
fn parse_claude(path: &Path) -> Result<Vec<AgentRun>> {
    let rdr = BufReader::new(std::fs::File::open(path)?);
    let mut main: Option<AgentRun> = None;
    let mut subs: Vec<AgentRun> = Vec::new();
    let mut session_id = String::new();
    let mut cwd = String::new();

    for line in rdr.lines().map_while(Result::ok) {
        let Ok(v) = serde_json::from_str::<Value>(&line) else {
            continue;
        };
        if let Some(s) = v.get("sessionId").and_then(|x| x.as_str()) {
            if session_id.is_empty() {
                session_id = s.to_string();
            }
        }
        if let Some(c) = v.get("cwd").and_then(|x| x.as_str()) {
            if cwd.is_empty() {
                cwd = c.to_string();
            }
        }
        let Some(when) = ts(&v, "timestamp") else {
            continue;
        };
        let sidechain = v
            .get("isSidechain")
            .and_then(|x| x.as_bool())
            .unwrap_or(false);

        let m = main.get_or_insert_with(|| AgentRun {
            source: "claude".into(),
            session_id: session_id.clone(),
            kind: "main".into(),
            is_sub: false,
            cwd: cwd.clone(),
            start: when,
            end: when,
            tool_calls: 0,
            input_tokens: 0,
            output_tokens: 0,
            file: String::new(),
        });
        if when < m.start {
            m.start = when;
        }
        if when > m.end {
            m.end = when;
        }
        if m.cwd.is_empty() && !cwd.is_empty() {
            m.cwd = cwd.clone();
        }

        if let Some(u) = v.pointer("/message/usage") {
            m.input_tokens += u
                .get("input_tokens")
                .and_then(|x| x.as_u64())
                .unwrap_or(0)
                + u.get("cache_read_input_tokens")
                    .and_then(|x| x.as_u64())
                    .unwrap_or(0);
            m.output_tokens += u.get("output_tokens").and_then(|x| x.as_u64()).unwrap_or(0);
        }

        if let Some(items) = v.pointer("/message/content").and_then(|c| c.as_array()) {
            for it in items {
                if it.get("type").and_then(|x| x.as_str()) != Some("tool_use") {
                    continue;
                }
                m.tool_calls += 1;
                let name = it.get("name").and_then(|x| x.as_str()).unwrap_or("");
                if sidechain || !(name == "Task" || name == "Agent") {
                    continue;
                }
                let kind = it
                    .pointer("/input/subagent_type")
                    .and_then(|x| x.as_str())
                    .unwrap_or("general-purpose")
                    .to_string();
                subs.push(AgentRun {
                    source: "claude".into(),
                    session_id: session_id.clone(),
                    kind,
                    is_sub: true,
                    cwd: cwd.clone(),
                    start: when,
                    end: when,
                    tool_calls: 0,
                    input_tokens: 0,
                    output_tokens: 0,
            file: String::new(),
                });
            }
        }
    }

    let mut out = Vec::new();
    if let Some(m) = main {
        if !m.cwd.is_empty() {
            for s in &mut subs {
                if s.cwd.is_empty() {
                    s.cwd = m.cwd.clone();
                }
            }
            out.push(m);
            out.append(&mut subs);
        }
    }
    Ok(out)
}

/// Codex: rollout JSONL. `session_meta` carries cwd; `token_count` carries a
/// running total; `function_call`s naming an agent are treated as subagents.
fn parse_codex(path: &Path) -> Result<Vec<AgentRun>> {
    let rdr = BufReader::new(std::fs::File::open(path)?);
    let mut main: Option<AgentRun> = None;
    let mut subs: Vec<AgentRun> = Vec::new();
    let mut cwd = String::new();
    let mut session_id = String::new();

    for line in rdr.lines().map_while(Result::ok) {
        let Ok(v) = serde_json::from_str::<Value>(&line) else {
            continue;
        };
        let Some(when) = ts(&v, "timestamp") else {
            continue;
        };
        let ptype = v
            .pointer("/payload/type")
            .and_then(|x| x.as_str())
            .unwrap_or("");

        if v.get("type").and_then(|x| x.as_str()) == Some("session_meta") {
            cwd = v
                .pointer("/payload/cwd")
                .and_then(|x| x.as_str())
                .unwrap_or("")
                .to_string();
            session_id = v
                .pointer("/payload/id")
                .and_then(|x| x.as_str())
                .unwrap_or("")
                .to_string();
        }
        if cwd.is_empty() {
            continue;
        }

        let m = main.get_or_insert_with(|| AgentRun {
            source: "codex".into(),
            session_id: session_id.clone(),
            kind: "main".into(),
            is_sub: false,
            cwd: cwd.clone(),
            start: when,
            end: when,
            tool_calls: 0,
            input_tokens: 0,
            output_tokens: 0,
            file: String::new(),
        });
        if when > m.end {
            m.end = when;
        }

        match ptype {
            "token_count" => {
                // Running totals: take the latest, don't accumulate.
                if let Some(t) = v.pointer("/payload/info/total_token_usage") {
                    m.input_tokens = t.get("input_tokens").and_then(|x| x.as_u64()).unwrap_or(0);
                    m.output_tokens = t.get("output_tokens").and_then(|x| x.as_u64()).unwrap_or(0);
                }
            }
            "function_call" | "custom_tool_call" | "mcp_tool_call_end" | "tool_search_call" => {
                m.tool_calls += 1;
                let name = v
                    .pointer("/payload/name")
                    .and_then(|x| x.as_str())
                    .unwrap_or("");
                // Only spawns create an agent. wait/list/close/interrupt_agent
                // are lifecycle calls against an already-spawned one.
                let spawn = matches!(
                    name.trim_start_matches('_'),
                    "spawn_agent" | "create_agent" | "start_agent"
                );
                if spawn {
                    let kind = v
                        .pointer("/payload/arguments")
                        .and_then(|x| x.as_str())
                        .and_then(|s| serde_json::from_str::<Value>(s).ok())
                        .and_then(|a| {
                            a.get("model")
                                .or_else(|| a.get("agent"))
                                .or_else(|| a.get("name"))
                                .and_then(|x| x.as_str())
                                .map(str::to_string)
                        })
                        .unwrap_or_else(|| "agent".to_string());
                    subs.push(AgentRun {
                        source: "codex".into(),
                        session_id: session_id.clone(),
                        kind,
                        is_sub: true,
                        cwd: cwd.clone(),
                        start: when,
                        end: when,
                        tool_calls: 0,
                        input_tokens: 0,
                        output_tokens: 0,
            file: String::new(),
                    });
                }
            }
            _ => {}
        }
    }

    let mut out = Vec::new();
    if let Some(m) = main {
        out.push(m);
        out.append(&mut subs);
    }
    Ok(out)
}

#[derive(Default, Clone)]
pub struct TaskAgents {
    pub sessions: u32,
    pub subagents: u32,
    pub tool_calls: u32,
    pub input_tokens: u64,
    pub output_tokens: u64,
    /// (kind, count) sorted by count desc
    pub by_kind: Vec<(String, u32)>,
    /// Individual runs credited to this task, newest first.
    pub runs: Vec<AgentRun>,
}

impl TaskAgents {
    pub fn total_agents(&self) -> u32 {
        self.sessions + self.subagents
    }
}

fn same_project(run_cwd: &str, project: &str) -> bool {
    run_cwd == project || run_cwd.starts_with(&format!("{}/", project.trim_end_matches('/')))
}

/// Attributes each run to exactly one task: the task whose active window
/// containing the run's start began most recently. Overlaps resolve to the
/// most recently started task, so a run is never double-counted.
pub fn attribute(tasks: &[crate::model::Task], runs: &[AgentRun]) -> HashMap<u64, TaskAgents> {
    let now = Utc::now();
    let mut out: HashMap<u64, TaskAgents> = HashMap::new();
    let mut kinds: HashMap<u64, HashMap<String, u32>> = HashMap::new();

    for r in runs {
        let mut best: Option<(DateTime<Utc>, u64)> = None;
        for t in tasks {
            if !same_project(&r.cwd, &t.project) {
                continue;
            }
            if let Some(ws) = t.overlaps(r.start, r.end, now) {
                if best.is_none_or(|(bs, _)| ws > bs) {
                    best = Some((ws, t.id));
                }
            }
        }
        let Some((_, id)) = best else { continue };
        let e = out.entry(id).or_default();
        if r.is_sub {
            e.subagents += 1;
        } else {
            e.sessions += 1;
        }
        e.tool_calls += r.tool_calls;
        e.input_tokens += r.input_tokens;
        e.output_tokens += r.output_tokens;
        e.runs.push(r.clone());
        let label = if r.is_sub {
            format!("{}:{}", r.source, r.kind)
        } else {
            format!("{}:session", r.source)
        };
        *kinds.entry(id).or_default().entry(label).or_insert(0) += 1;
    }

    for (id, k) in kinds {
        let mut v: Vec<(String, u32)> = k.into_iter().collect();
        v.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(&b.0)));
        if let Some(e) = out.get_mut(&id) {
            e.by_kind = v;
        }
    }
    for e in out.values_mut() {
        e.runs.sort_by(|a, b| b.start.cmp(&a.start));
    }
    out
}
