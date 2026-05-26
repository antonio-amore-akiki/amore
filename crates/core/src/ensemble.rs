// Multi-agent ensemble (S14a). Architect + Skeptic wired now; remaining 4 roles
// + EIG + SQLite persistence + credit assignment land in S14b/c. Generic
// LlmClient trait so tests use deterministic MockLlm, production uses Ollama.

use anyhow::Result;
use std::future::Future;

pub trait LlmClient: Send + Sync {
    fn generate(
        &self,
        system: Option<&str>,
        prompt: &str,
    ) -> impl Future<Output = Result<String>> + Send;
}

impl LlmClient for crate::ollama::OllamaClient {
    fn generate(
        &self,
        system: Option<&str>,
        prompt: &str,
    ) -> impl Future<Output = Result<String>> + Send {
        crate::ollama::OllamaClient::generate(self, system, prompt)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AgentRole {
    Architect,
    Skeptic,
    Historian,
    Reviewer,
    Negotiator,
    Implementer,
}

const VOTE_SCHEMA: &str = "Respond with ONE line JSON: {\"position\":\"approve\"|\"reject\"|\"abstain\",\"confidence\":<0..1>,\"rationale\":\"<short>\"}.";

impl AgentRole {
    pub fn name(&self) -> &'static str {
        match self {
            AgentRole::Architect => "architect",
            AgentRole::Skeptic => "skeptic",
            AgentRole::Historian => "historian",
            AgentRole::Reviewer => "reviewer",
            AgentRole::Negotiator => "negotiator",
            AgentRole::Implementer => "implementer",
        }
    }

    pub fn system_prompt(&self) -> String {
        let head = match self {
            AgentRole::Architect => {
                "You are the Architect agent. Evaluate a proposed change for soundness, scope, and architectural fit. Default to abstain when evidence is missing."
            }
            AgentRole::Skeptic => {
                "You are the Skeptic agent. Challenge the proposed change: probe assumptions, demand evidence, surface hidden risks. Reject when any load-bearing claim is unproven."
            }
            AgentRole::Historian => {
                "You are the Historian agent. Search precedent: has this been tried? What did past sessions decide on similar matters?"
            }
            AgentRole::Reviewer => {
                "You are the Reviewer agent. Judge correctness, regression risk, and release-readiness."
            }
            AgentRole::Negotiator => {
                "You are the Negotiator agent. Identify the single highest-expected-information-gain clarifying question."
            }
            AgentRole::Implementer => {
                "You are the Implementer agent. Estimate whether the change is executable in a bounded edit budget."
            }
        };
        format!("{head} {VOTE_SCHEMA}")
    }
}

pub struct LlmAgent<L: LlmClient> {
    pub role: AgentRole,
    pub llm: std::sync::Arc<L>,
}

impl<L: LlmClient> LlmAgent<L> {
    pub fn new(role: AgentRole, llm: std::sync::Arc<L>) -> Self {
        Self { role, llm }
    }

    pub async fn vote(&self, decision_prompt: &str) -> Result<Vote> {
        let sys = self.role.system_prompt();
        let raw = self.llm.generate(Some(&sys), decision_prompt).await?;
        Ok(parse_vote(self.role.name(), &raw))
    }
}

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct Vote {
    pub agent: String,
    pub position: String,
    pub rationale: String,
    pub confidence: f32,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct Decision {
    pub decision_id: String,
    pub consensus: String,
    pub votes: Vec<Vote>,
    pub confidence: f32,
}

/// Parse a vote JSON line. Tolerates prose around the JSON; falls back to
/// abstain on parse failure (never panics).
pub fn parse_vote(agent: &str, raw: &str) -> Vote {
    let json_slice = extract_first_json_object(raw);
    let parsed: Option<serde_json::Value> = json_slice
        .as_deref()
        .and_then(|s| serde_json::from_str(s).ok());
    let (position, confidence, rationale) = match parsed {
        Some(v) => (
            v.get("position")
                .and_then(|x| x.as_str())
                .map(canonical_position)
                .unwrap_or_else(|| "abstain".to_string()),
            v.get("confidence")
                .and_then(|x| x.as_f64())
                .unwrap_or(0.0)
                .clamp(0.0, 1.0) as f32,
            v.get("rationale")
                .and_then(|x| x.as_str())
                .unwrap_or("")
                .to_string(),
        ),
        None => ("abstain".to_string(), 0.0, raw.trim().to_string()),
    };
    Vote {
        agent: agent.to_string(),
        position,
        rationale,
        confidence,
    }
}

fn canonical_position(s: &str) -> String {
    let lower = s.trim().to_lowercase();
    if lower.starts_with("approv") || lower == "yes" || lower == "yea" {
        "approve".to_string()
    } else if lower.starts_with("reject") || lower == "no" || lower == "nay" {
        "reject".to_string()
    } else {
        "abstain".to_string()
    }
}

fn extract_first_json_object(s: &str) -> Option<String> {
    let bytes = s.as_bytes();
    let start = bytes.iter().position(|&b| b == b'{')?;
    let mut depth = 0i32;
    let mut in_str = false;
    let mut esc = false;
    for (i, &b) in bytes.iter().enumerate().skip(start) {
        let c = b as char;
        if in_str {
            if esc {
                esc = false;
            } else if c == '\\' {
                esc = true;
            } else if c == '"' {
                in_str = false;
            }
            continue;
        }
        match c {
            '"' => in_str = true,
            '{' => depth += 1,
            '}' => {
                depth -= 1;
                if depth == 0 {
                    return Some(s[start..=i].to_string());
                }
            }
            _ => {}
        }
    }
    None
}

pub struct Orchestrator<L: LlmClient> {
    agents: Vec<LlmAgent<L>>,
}

impl<L: LlmClient + 'static> Orchestrator<L> {
    pub fn new(agents: Vec<LlmAgent<L>>) -> Self {
        Self { agents }
    }

    /// Fan out the decision prompt to every agent (sequential await for now —
    /// parallel join_all lands in S14b once futures crate is added to the
    /// workspace), collect votes, tally confidence-weighted majority.
    /// `decision_id` is the caller-supplied stable id used for later credit-
    /// assignment correlation (S14b).
    pub async fn decide(&self, decision_id: &str, decision_prompt: &str) -> Result<Decision> {
        let mut votes = Vec::with_capacity(self.agents.len());
        for a in &self.agents {
            votes.push(a.vote(decision_prompt).await?);
        }
        let (consensus, confidence) = tally(&votes);
        Ok(Decision {
            decision_id: decision_id.to_string(),
            consensus,
            votes,
            confidence,
        })
    }
}

/// Confidence-weighted majority. Sums each position's confidence; the largest
/// sum wins. Tie -> abstain. Empty input -> ("abstain", 0.0). Confidence is
/// winner_weight / total.
pub fn tally(votes: &[Vote]) -> (String, f32) {
    if votes.is_empty() {
        return ("abstain".to_string(), 0.0);
    }
    let mut approve = 0.0f32;
    let mut reject = 0.0f32;
    let mut abstain = 0.0f32;
    for v in votes {
        match v.position.as_str() {
            "approve" => approve += v.confidence,
            "reject" => reject += v.confidence,
            _ => abstain += v.confidence,
        }
    }
    let total = approve + reject + abstain;
    if total <= 0.0 {
        return ("abstain".to_string(), 0.0);
    }
    let (winner, w) = if approve > reject && approve > abstain {
        ("approve", approve)
    } else if reject > approve && reject > abstain {
        ("reject", reject)
    } else {
        ("abstain", abstain.max(approve.max(reject)))
    };
    (winner.to_string(), (w / total).clamp(0.0, 1.0))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;

    struct MockLlm {
        responses: std::sync::Mutex<std::collections::HashMap<&'static str, String>>,
    }
    impl MockLlm {
        fn new() -> Self {
            Self {
                responses: std::sync::Mutex::new(std::collections::HashMap::new()),
            }
        }
        fn set(&self, role: &'static str, body: &str) {
            self.responses
                .lock()
                .unwrap()
                .insert(role, body.to_string());
        }
    }
    impl LlmClient for MockLlm {
        fn generate(
            &self,
            system: Option<&str>,
            _prompt: &str,
        ) -> impl Future<Output = Result<String>> + Send {
            let role = system
                .map(|s| {
                    if s.contains("Architect") {
                        "architect"
                    } else if s.contains("Skeptic") {
                        "skeptic"
                    } else if s.contains("Reviewer") {
                        "reviewer"
                    } else {
                        "unknown"
                    }
                })
                .unwrap_or("unknown");
            let body = self
                .responses
                .lock()
                .unwrap()
                .get(role)
                .cloned()
                .unwrap_or_else(|| {
                    "{\"position\":\"abstain\",\"confidence\":0.0,\"rationale\":\"no canned\"}"
                        .to_string()
                });
            async move { Ok(body) }
        }
    }

    fn v(agent: &str, pos: &str, conf: f32) -> Vote {
        Vote {
            agent: agent.into(),
            position: pos.into(),
            rationale: "".into(),
            confidence: conf,
        }
    }

    #[test]
    fn parse_vote_strict_json() {
        let r = parse_vote(
            "a",
            "{\"position\":\"approve\",\"confidence\":0.9,\"rationale\":\"sound\"}",
        );
        assert_eq!(r.position, "approve");
        assert!((r.confidence - 0.9).abs() < 1e-6);
        assert_eq!(r.rationale, "sound");
    }

    #[test]
    fn parse_vote_tolerates_prose_around_json() {
        let r = parse_vote(
            "s",
            "Verdict:\n{\"position\":\"reject\",\"confidence\":0.7,\"rationale\":\"weak\"}\nThat's all.",
        );
        assert_eq!(r.position, "reject");
    }

    #[test]
    fn parse_vote_garbage_falls_back_to_abstain() {
        let r = parse_vote("a", "the model went off-script");
        assert_eq!(r.position, "abstain");
        assert_eq!(r.confidence, 0.0);
    }

    #[test]
    fn parse_vote_clamps_oof_confidence() {
        let r = parse_vote(
            "a",
            "{\"position\":\"approve\",\"confidence\":1.7,\"rationale\":\"\"}",
        );
        assert_eq!(r.confidence, 1.0);
    }

    #[test]
    fn tally_unanimous_approve() {
        let (p, c) = tally(&[v("a", "approve", 0.9), v("b", "approve", 0.7)]);
        assert_eq!(p, "approve");
        assert!((c - 1.0).abs() < 1e-6);
    }

    #[test]
    fn tally_confidence_weighted_majority() {
        let (p, _) = tally(&[
            v("a", "approve", 0.2),
            v("b", "approve", 0.3),
            v("c", "reject", 0.9),
        ]);
        assert_eq!(p, "reject");
    }

    #[test]
    fn tally_empty_returns_abstain() {
        let (p, c) = tally(&[]);
        assert_eq!(p, "abstain");
        assert_eq!(c, 0.0);
    }

    #[test]
    fn orchestrator_decide_aggregates_two_agents() {
        let llm = Arc::new(MockLlm::new());
        llm.set(
            "architect",
            "{\"position\":\"approve\",\"confidence\":0.8,\"rationale\":\"good\"}",
        );
        llm.set(
            "skeptic",
            "{\"position\":\"approve\",\"confidence\":0.6,\"rationale\":\"ok\"}",
        );
        let orch = Orchestrator::new(vec![
            LlmAgent::new(AgentRole::Architect, llm.clone()),
            LlmAgent::new(AgentRole::Skeptic, llm.clone()),
        ]);
        let rt = tokio::runtime::Runtime::new().unwrap();
        let d = rt.block_on(orch.decide("d-1", "Refactor X?")).unwrap();
        assert_eq!(d.consensus, "approve");
        assert_eq!(d.votes.len(), 2);
        assert_eq!(d.decision_id, "d-1");
        assert!(d.confidence > 0.0);
    }

    #[test]
    fn orchestrator_decide_with_disagreement() {
        let llm = Arc::new(MockLlm::new());
        llm.set(
            "architect",
            "{\"position\":\"approve\",\"confidence\":0.5,\"rationale\":\"ok\"}",
        );
        llm.set(
            "skeptic",
            "{\"position\":\"reject\",\"confidence\":0.95,\"rationale\":\"unproven\"}",
        );
        let orch = Orchestrator::new(vec![
            LlmAgent::new(AgentRole::Architect, llm.clone()),
            LlmAgent::new(AgentRole::Skeptic, llm.clone()),
        ]);
        let rt = tokio::runtime::Runtime::new().unwrap();
        let d = rt.block_on(orch.decide("d-2", "Add dep X?")).unwrap();
        assert_eq!(d.consensus, "reject");
    }

    #[test]
    fn role_system_prompts_are_distinct() {
        let a = AgentRole::Architect.system_prompt();
        let s = AgentRole::Skeptic.system_prompt();
        let h = AgentRole::Historian.system_prompt();
        assert!(a.contains("Architect") && s.contains("Skeptic") && h.contains("Historian"));
        assert_ne!(a, s);
        assert_ne!(s, h);
    }
}
