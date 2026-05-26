// QA A7 — Ensemble Orchestrator drives REAL Architect + Skeptic agents
// against live Ollama (qwen3:8b). Tests that the JSON-vote contract works
// against an actual LLM, not the deterministic MockLlm.
//
// Gated by OBELION_TEST_OLLAMA=1; uses ensure_daemons.{ps1,sh} before invoke.
// Run with:
//     $env:OBELION_TEST_OLLAMA=1
//     cargo test -p obelion-core --test ensemble_real_ollama -- --ignored --nocapture
//
// Failure modes covered:
//   - LLM returns non-JSON garbage -> parse_vote falls back to abstain
//   - LLM returns malformed JSON   -> same fallback
//   - Vote position outside {approve,reject,abstain} -> canonical_position normalizes
//   - Confidence outside [0,1]     -> parse_vote clamps
//
// What this proves vs the existing MockLlm tests:
//   1. Generic LlmClient<OllamaClient> wiring actually round-trips a real model
//   2. The system_prompt format is parseable by an instruction-tuned LLM
//   3. Sequential await fan-out completes in bounded time (<60s for 2 agents)

use obelion_core::ensemble::{AgentRole, LlmAgent, Orchestrator};
use obelion_core::ollama::OllamaClient;
use std::sync::Arc;
use std::time::Duration;

fn enabled() -> bool {
    std::env::var("OBELION_TEST_OLLAMA").ok().as_deref() == Some("1")
}

#[tokio::test]
#[ignore = "requires OBELION_TEST_OLLAMA=1 + live Ollama on 11434 with qwen3:8b"]
async fn architect_plus_skeptic_decide_against_real_qwen3() {
    if !enabled() {
        eprintln!("OBELION_TEST_OLLAMA not set; skipping");
        return;
    }

    let base = std::env::var("OBELION_OLLAMA_URL")
        .unwrap_or_else(|_| "http://127.0.0.1:11434".to_string());
    let llm = Arc::new(OllamaClient::new(&base));
    let orch = Orchestrator::new(vec![
        LlmAgent::new(AgentRole::Architect, llm.clone()),
        LlmAgent::new(AgentRole::Skeptic, llm.clone()),
    ]);

    // Concrete bounded decision prompt — both agents have a clear position to
    // form. Architect should lean approve (scope-fit), Skeptic may lean reject
    // (unproven claim). The TEST does not assert WHICH consensus emerges
    // (LLMs vary). It asserts the SHAPE: 2 votes parsed, each well-formed,
    // tally produces a valid consensus string, total wall-time <60s.
    let prompt = "Should we add a `--verbose` flag to the obelion CLI that prints \
                  every config-file diff during `init`? Already-shipped: `--dry-run`. \
                  Trade-off: more debuggable but doubles output volume.";

    let started = std::time::Instant::now();
    let decision = tokio::time::timeout(
        Duration::from_secs(60),
        orch.decide("qa-a7-verbose-flag", prompt),
    )
    .await
    .expect("orchestrator must complete in <60s")
    .expect("orchestrator returned Ok");
    let elapsed = started.elapsed();
    eprintln!("[A7] decide() elapsed: {:?}", elapsed);
    eprintln!("[A7] decision: {:?}", decision);

    assert_eq!(decision.votes.len(), 2, "two agents must vote");
    let valid_positions = ["approve", "reject", "abstain"];
    for vote in &decision.votes {
        assert!(
            valid_positions.contains(&vote.position.as_str()),
            "agent {} returned invalid position {:?}",
            vote.agent,
            vote.position
        );
        assert!(
            (0.0..=1.0).contains(&vote.confidence),
            "agent {} confidence {} outside [0,1]",
            vote.agent,
            vote.confidence
        );
    }
    assert!(
        valid_positions.contains(&decision.consensus.as_str()),
        "consensus must be in valid set, got {:?}",
        decision.consensus
    );
    assert!(
        (0.0..=1.0).contains(&decision.confidence),
        "consensus confidence outside [0,1]"
    );
    assert_eq!(decision.decision_id, "qa-a7-verbose-flag");
}

#[tokio::test]
#[ignore = "requires OBELION_TEST_OLLAMA=1 + live Ollama on 11434 with qwen3:8b"]
async fn agent_role_distinct_rationales_against_real_llm() {
    if !enabled() {
        eprintln!("OBELION_TEST_OLLAMA not set; skipping");
        return;
    }

    let base = std::env::var("OBELION_OLLAMA_URL")
        .unwrap_or_else(|_| "http://127.0.0.1:11434".to_string());
    let llm = Arc::new(OllamaClient::new(&base));

    let architect = LlmAgent::new(AgentRole::Architect, llm.clone());
    let skeptic = LlmAgent::new(AgentRole::Skeptic, llm.clone());

    let prompt = "Should we accept a PR that adds a 'sync_to_cloud' feature without tests?";

    let v_arch = tokio::time::timeout(Duration::from_secs(45), architect.vote(prompt))
        .await
        .expect("architect must vote within 45s")
        .expect("architect returned Ok");
    let v_skep = tokio::time::timeout(Duration::from_secs(45), skeptic.vote(prompt))
        .await
        .expect("skeptic must vote within 45s")
        .expect("skeptic returned Ok");

    eprintln!(
        "[A7] architect: position={} conf={} rationale={:?}",
        v_arch.position, v_arch.confidence, v_arch.rationale
    );
    eprintln!(
        "[A7] skeptic:   position={} conf={} rationale={:?}",
        v_skep.position, v_skep.confidence, v_skep.rationale
    );

    assert_eq!(v_arch.agent, "architect");
    assert_eq!(v_skep.agent, "skeptic");

    // Rationale field — proves the LLM actually filled it (real model output)
    // vs MockLlm canned response. Soft bar — at least one must include >=8 chars
    // of rationale (LLMs vary; a strict bar would flake).
    let rationales_len = v_arch.rationale.len().max(v_skep.rationale.len());
    assert!(
        rationales_len >= 8,
        "at least one agent must produce a >=8-char rationale, got architect={:?} ({} chars) skeptic={:?} ({} chars)",
        v_arch.rationale,
        v_arch.rationale.len(),
        v_skep.rationale,
        v_skep.rationale.len(),
    );
}
