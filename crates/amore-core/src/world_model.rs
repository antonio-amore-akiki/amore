// World model — persistent typed graph (projects + tool_reliability + preferences).
// Schema idempotent on open; separate DB file from observation chain.

use anyhow::Result;
use rusqlite::{Connection, OptionalExtension, params};
use std::path::Path;
use std::sync::Mutex;

pub struct WorldModel {
    conn: Mutex<Connection>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ProjectNode {
    pub name: String,
    pub payload: serde_json::Value,
    pub updated_ts: i64,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ProjectEdge {
    pub from: String,
    pub to: String,
    pub edge_type: String,
    pub weight: f64,
}

#[derive(Debug, Clone)]
pub struct ToolReliability {
    pub tool: String,
    pub class: String,
    pub success_count: u64,
    pub failure_count: u64,
}

impl ToolReliability {
    /// Laplace-smoothed: (s+1)/(s+f+2). Avoids 0/0 and 1.0-spike on first hit.
    pub fn success_rate(&self) -> f64 {
        let s = self.success_count as f64;
        let f = self.failure_count as f64;
        (s + 1.0) / (s + f + 2.0)
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct PreferenceNode {
    pub key: String,
    pub evidence_count: u64,
    pub probability: f64,
    pub last_evidence: Option<String>,
    pub updated_ts: i64,
}

impl WorldModel {
    pub fn open(path: &Path) -> Result<Self> {
        Self::from_conn(Connection::open(path)?)
    }
    pub fn open_in_memory() -> Result<Self> {
        Self::from_conn(Connection::open_in_memory()?)
    }
    fn from_conn(conn: Connection) -> Result<Self> {
        let wm = Self {
            conn: Mutex::new(conn),
        };
        wm.init_schema()?;
        Ok(wm)
    }

    fn init_schema(&self) -> Result<()> {
        self.conn.lock().expect("mutex poisoned: unrecoverable state corruption").execute_batch(
            r#"
            CREATE TABLE IF NOT EXISTS wm_projects (
                name TEXT PRIMARY KEY,
                payload TEXT NOT NULL,
                updated_ts INTEGER NOT NULL
            );
            CREATE TABLE IF NOT EXISTS wm_project_edges (
                from_name TEXT NOT NULL,
                to_name TEXT NOT NULL,
                edge_type TEXT NOT NULL,
                weight REAL NOT NULL DEFAULT 1.0,
                PRIMARY KEY (from_name, to_name, edge_type)
            );
            CREATE INDEX IF NOT EXISTS idx_wm_edges_from ON wm_project_edges(from_name);
            CREATE INDEX IF NOT EXISTS idx_wm_edges_to   ON wm_project_edges(to_name);
            CREATE TABLE IF NOT EXISTS wm_tool_reliability (
                tool_name TEXT NOT NULL,
                class TEXT NOT NULL,
                success_count INTEGER NOT NULL DEFAULT 0,
                failure_count INTEGER NOT NULL DEFAULT 0,
                updated_ts INTEGER NOT NULL,
                PRIMARY KEY (tool_name, class)
            );
            CREATE TABLE IF NOT EXISTS wm_revealed_preferences (
                pref_key TEXT PRIMARY KEY,
                evidence_count INTEGER NOT NULL DEFAULT 0,
                probability REAL NOT NULL DEFAULT 0.5,
                last_evidence TEXT,
                updated_ts INTEGER NOT NULL
            );
            "#,
        )?;
        Ok(())
    }

    pub fn upsert_project(&self, name: &str, payload: serde_json::Value) -> Result<()> {
        let ts = now_unix_ms();
        let conn = self.conn.lock().expect("mutex poisoned: unrecoverable state corruption");
        conn.execute(
            "INSERT INTO wm_projects (name, payload, updated_ts) VALUES (?1, ?2, ?3) \
             ON CONFLICT(name) DO UPDATE SET payload=excluded.payload, updated_ts=excluded.updated_ts",
            params![name, payload.to_string(), ts],
        )?;
        Ok(())
    }

    pub fn get_project(&self, name: &str) -> Result<Option<ProjectNode>> {
        let conn = self.conn.lock().expect("mutex poisoned: unrecoverable state corruption");
        let row: Option<(String, i64)> = conn
            .query_row(
                "SELECT payload, updated_ts FROM wm_projects WHERE name = ?1",
                params![name],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .optional()?;
        Ok(row.map(|(payload, ts)| ProjectNode {
            name: name.to_string(),
            payload: serde_json::from_str(&payload).unwrap_or(serde_json::Value::Null),
            updated_ts: ts,
        }))
    }

    pub fn add_project_edge(
        &self,
        from: &str,
        to: &str,
        edge_type: &str,
        weight: f64,
    ) -> Result<()> {
        let conn = self.conn.lock().expect("mutex poisoned: unrecoverable state corruption");
        conn.execute(
            "INSERT INTO wm_project_edges (from_name, to_name, edge_type, weight) \
             VALUES (?1, ?2, ?3, ?4) \
             ON CONFLICT(from_name, to_name, edge_type) DO UPDATE SET weight=excluded.weight",
            params![from, to, edge_type, weight],
        )?;
        Ok(())
    }

    pub fn project_neighbors(&self, name: &str) -> Result<Vec<ProjectEdge>> {
        let conn = self.conn.lock().expect("mutex poisoned: unrecoverable state corruption");
        let mut stmt = conn.prepare(
            "SELECT from_name, to_name, edge_type, weight FROM wm_project_edges \
             WHERE from_name = ?1 ORDER BY weight DESC, to_name ASC",
        )?;
        let edges = stmt
            .query_map(params![name], |r| {
                Ok(ProjectEdge {
                    from: r.get(0)?,
                    to: r.get(1)?,
                    edge_type: r.get(2)?,
                    weight: r.get(3)?,
                })
            })?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        Ok(edges)
    }

    pub fn record_tool_outcome(&self, tool: &str, class: &str, success: bool) -> Result<()> {
        let ts = now_unix_ms();
        let (s_inc, f_inc) = if success { (1i64, 0i64) } else { (0i64, 1i64) };
        let conn = self.conn.lock().expect("mutex poisoned: unrecoverable state corruption");
        conn.execute(
            "INSERT INTO wm_tool_reliability (tool_name, class, success_count, failure_count, updated_ts) \
             VALUES (?1, ?2, ?3, ?4, ?5) \
             ON CONFLICT(tool_name, class) DO UPDATE SET \
               success_count = wm_tool_reliability.success_count + ?3, \
               failure_count = wm_tool_reliability.failure_count + ?4, \
               updated_ts = ?5",
            params![tool, class, s_inc, f_inc, ts],
        )?;
        Ok(())
    }

    pub fn tool_reliability(&self, tool: &str, class: &str) -> Result<Option<ToolReliability>> {
        let conn = self.conn.lock().expect("mutex poisoned: unrecoverable state corruption");
        conn.query_row(
            "SELECT success_count, failure_count FROM wm_tool_reliability \
             WHERE tool_name = ?1 AND class = ?2",
            params![tool, class],
            |r| {
                let s: i64 = r.get(0)?;
                let f: i64 = r.get(1)?;
                Ok(ToolReliability {
                    tool: tool.to_string(),
                    class: class.to_string(),
                    success_count: s.max(0) as u64,
                    failure_count: f.max(0) as u64,
                })
            },
        )
        .optional()
        .map_err(anyhow::Error::from)
    }

    /// Bayesian update in log-odds space. `lift` > 0 -> toward 1, < 0 -> toward 0.
    /// Step magnitude clamped to [-3, 3] for early-evidence stability.
    pub fn update_preference(&self, key: &str, evidence: &str, lift: f64) -> Result<f64> {
        let conn = self.conn.lock().expect("mutex poisoned: unrecoverable state corruption");
        let current: Option<(i64, f64)> = conn
            .query_row(
                "SELECT evidence_count, probability FROM wm_revealed_preferences WHERE pref_key = ?1",
                params![key],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .optional()?;
        let (cur_n, cur_p) = current.unwrap_or((0, 0.5));
        let eps = 1e-6_f64;
        let p_clamped = cur_p.clamp(eps, 1.0 - eps);
        let lift_clamped = lift.clamp(-3.0, 3.0);
        let log_odds = (p_clamped / (1.0 - p_clamped)).ln() + lift_clamped;
        let new_p = 1.0 / (1.0 + (-log_odds).exp());
        let new_n = cur_n + 1;
        let ts = now_unix_ms();
        conn.execute(
            "INSERT INTO wm_revealed_preferences (pref_key, evidence_count, probability, last_evidence, updated_ts) \
             VALUES (?1, ?2, ?3, ?4, ?5) \
             ON CONFLICT(pref_key) DO UPDATE SET \
               evidence_count = excluded.evidence_count, \
               probability = excluded.probability, \
               last_evidence = excluded.last_evidence, \
               updated_ts = excluded.updated_ts",
            params![key, new_n, new_p, evidence, ts],
        )?;
        Ok(new_p)
    }

    pub fn get_preference(&self, key: &str) -> Result<Option<PreferenceNode>> {
        let conn = self.conn.lock().expect("mutex poisoned: unrecoverable state corruption");
        conn.query_row(
            "SELECT evidence_count, probability, last_evidence, updated_ts \
             FROM wm_revealed_preferences WHERE pref_key = ?1",
            params![key],
            |r| {
                let n: i64 = r.get(0)?;
                Ok(PreferenceNode {
                    key: key.to_string(),
                    evidence_count: n.max(0) as u64,
                    probability: r.get(1)?,
                    last_evidence: r.get(2)?,
                    updated_ts: r.get(3)?,
                })
            },
        )
        .optional()
        .map_err(anyhow::Error::from)
    }

    pub fn top_preferences(&self, top_n: u64) -> Result<Vec<PreferenceNode>> {
        let conn = self.conn.lock().expect("mutex poisoned: unrecoverable state corruption");
        let mut stmt = conn.prepare(
            "SELECT pref_key, evidence_count, probability, last_evidence, updated_ts \
             FROM wm_revealed_preferences \
             ORDER BY probability DESC, evidence_count DESC, pref_key ASC \
             LIMIT ?1",
        )?;
        let rows = stmt
            .query_map(params![top_n as i64], |r| {
                let n: i64 = r.get(1)?;
                Ok(PreferenceNode {
                    key: r.get(0)?,
                    evidence_count: n.max(0) as u64,
                    probability: r.get(2)?,
                    last_evidence: r.get(3)?,
                    updated_ts: r.get(4)?,
                })
            })?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        Ok(rows)
    }
}

fn now_unix_ms() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn wm() -> WorldModel {
        WorldModel::open_in_memory().unwrap()
    }

    #[test]
    fn upsert_then_get_project_roundtrip() {
        let wm = wm();
        wm.upsert_project("amore", json!({"lang":"rust"})).unwrap();
        let p = wm.get_project("amore").unwrap().unwrap();
        assert_eq!(p.name, "amore");
        assert_eq!(p.payload["lang"], "rust");
    }

    #[test]
    fn upsert_overwrites_existing() {
        let wm = wm();
        wm.upsert_project("amore", json!({"v":1})).unwrap();
        wm.upsert_project("amore", json!({"v":2})).unwrap();
        assert_eq!(wm.get_project("amore").unwrap().unwrap().payload["v"], 2);
    }

    #[test]
    fn project_edges_are_queryable_by_source() {
        let wm = wm();
        wm.upsert_project("a", json!({})).unwrap();
        wm.upsert_project("b", json!({})).unwrap();
        wm.upsert_project("c", json!({})).unwrap();
        wm.add_project_edge("a", "b", "depends_on", 1.0).unwrap();
        wm.add_project_edge("a", "c", "shares_dep", 0.5).unwrap();
        let n = wm.project_neighbors("a").unwrap();
        assert_eq!(n.len(), 2);
        assert_eq!(n[0].to, "b");
        assert_eq!(n[1].to, "c");
    }

    #[test]
    fn project_edge_upsert_overwrites_weight() {
        let wm = wm();
        wm.add_project_edge("a", "b", "depends_on", 0.5).unwrap();
        wm.add_project_edge("a", "b", "depends_on", 0.9).unwrap();
        let n = wm.project_neighbors("a").unwrap();
        assert_eq!(n.len(), 1);
        assert!((n[0].weight - 0.9).abs() < 1e-9);
    }

    #[test]
    fn tool_reliability_accumulates_outcomes() {
        let wm = wm();
        for _ in 0..8 {
            wm.record_tool_outcome("Bash", "file_ops", true).unwrap();
        }
        for _ in 0..2 {
            wm.record_tool_outcome("Bash", "file_ops", false).unwrap();
        }
        let r = wm.tool_reliability("Bash", "file_ops").unwrap().unwrap();
        assert_eq!(r.success_count, 8);
        assert_eq!(r.failure_count, 2);
        assert!((r.success_rate() - 0.75).abs() < 1e-9);
    }

    #[test]
    fn missing_tool_reliability_returns_none() {
        assert!(
            wm().tool_reliability("Bash", "uninstall")
                .unwrap()
                .is_none()
        );
    }

    #[test]
    fn preference_update_moves_toward_one_with_positive_lift() {
        let wm = wm();
        let p1 = wm
            .update_preference("elite-engineering", "s1", 1.0)
            .unwrap();
        let p2 = wm
            .update_preference("elite-engineering", "s2", 1.0)
            .unwrap();
        assert!(p1 > 0.5);
        assert!(p2 > p1);
        let node = wm.get_preference("elite-engineering").unwrap().unwrap();
        assert_eq!(node.evidence_count, 2);
        assert_eq!(node.last_evidence.unwrap(), "s2");
    }

    #[test]
    fn preference_update_moves_toward_zero_with_negative_lift() {
        assert!(wm().update_preference("verbose", "s1", -2.0).unwrap() < 0.5);
    }

    #[test]
    fn top_preferences_orders_by_probability_desc() {
        let wm = wm();
        wm.update_preference("a", "s", 3.0).unwrap();
        wm.update_preference("b", "s", 1.0).unwrap();
        wm.update_preference("c", "s", -1.0).unwrap();
        let top = wm.top_preferences(10).unwrap();
        assert_eq!(top.len(), 3);
        assert_eq!(top[0].key, "a");
        assert!(top[0].probability > top[1].probability);
    }
}
