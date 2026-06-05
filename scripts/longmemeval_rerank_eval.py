#!/usr/bin/env python3
"""longmemeval_rerank_eval.py — Python LongMemEval-S R@5 with MiniLM cross-encoder rerank.

Closes reranker-b7-live-r5-measurement known_gap autonomously by bypassing the
Rust ort 2.0.0-rc.12 session-init hang (Python onnxruntime 1.26.0 loads MiniLM
in 0.21s vs >7min in Rust). Same model + same corpus + same metric — measures
the rerank lift over RRF-only baseline directly.

Pipeline per query:
  1. Embed query via Ollama (nomic-embed-text, same as Rust runner)
  2. Embed each haystack session text
  3. Take top-K1 (30) by cosine — vector lane
  4. Take top-K1 by BM25 (rank-bm25 BM25Okapi) — bm25 lane
  5. RRF fuse to top-K2 (20) candidates
  6. MiniLM cross-encoder rerank to top-10
  7. Compute R@1 / R@5 / R@10 / MRR against `answer_session_ids`

Run:
  pip install onnxruntime transformers rank-bm25 numpy requests
  ollama serve  &  ollama pull nomic-embed-text
  python scripts/longmemeval_rerank_eval.py --corpus state/longmemeval-s/test.jsonl --subset 20 \
    --out state/longmemeval-python-reranked-subset20.json

Output JSON includes overall RRF-only AND RRF+rerank scores so the lift is directly observable.

Prior-art verdict: state/prior-art-verdict.json (replaces crates/amore-eval/src/bin/longmemeval_runner.rs
for the reranker-enabled measurement path only; non-rerank path retained in Rust).
"""
import argparse
import json
import sys
import time
from pathlib import Path
import numpy as np
import requests

OLLAMA_URL = "http://127.0.0.1:11434"
EMBED_MODEL = "nomic-embed-text"
RRF_K = 60
TOP_K1 = 30
TOP_K2 = 20
TOP_K_FINAL = 10


def embed(text):
    r = requests.post(
        f"{OLLAMA_URL}/api/embeddings",
        json={"model": EMBED_MODEL, "prompt": text[:1500]},
        timeout=30,
    )
    r.raise_for_status()
    v = np.array(r.json()["embedding"], dtype=np.float32)
    return v / (np.linalg.norm(v) + 1e-9)


def rrf_fuse(rankings, k=RRF_K, top=TOP_K2):
    scores = {}
    for ranking in rankings:
        for rank, sid in enumerate(ranking):
            scores[sid] = scores.get(sid, 0.0) + 1.0 / (k + rank + 1)
    return [s for s, _ in sorted(scores.items(), key=lambda x: -x[1])[:top]]


def bm25_rank(query, docs, top=TOP_K1):
    from rank_bm25 import BM25Okapi
    tokenized = [t.lower().split() for _, t in docs]
    bm = BM25Okapi(tokenized)
    scores = bm.get_scores(query.lower().split())
    ranked = sorted(zip([s for s, _ in docs], scores), key=lambda x: -x[1])
    return [s for s, _ in ranked[:top]]


def vector_rank(qv, sid_vecs, top=TOP_K1):
    sims = [(sid, float(qv @ v)) for sid, v in sid_vecs]
    sims.sort(key=lambda x: -x[1])
    return [s for s, _ in sims[:top]]


def truncate_bytes(s, max_bytes=1500):
    b = s.encode("utf-8")
    if len(b) <= max_bytes:
        return s
    return b[:max_bytes].decode("utf-8", errors="ignore")


class Reranker:
    def __init__(self, model_path, tokenizer_path):
        import onnxruntime as ort
        from transformers import AutoTokenizer
        t0 = time.time()
        self.session = ort.InferenceSession(model_path, providers=["CPUExecutionProvider"])
        # Inspect actual input signature — bge-reranker-base takes (input_ids, attention_mask)
        # while MiniLM-cross-encoder also takes token_type_ids. Use a set so we feed only what
        # the model expects.
        self.expected_inputs = {i.name for i in self.session.get_inputs()}
        try:
            self.tokenizer = AutoTokenizer.from_pretrained(str(Path(tokenizer_path).parent))
        except Exception:
            self.tokenizer = AutoTokenizer.from_pretrained("cross-encoder/ms-marco-MiniLM-L-6-v2")
        # MiniLM tokenizer.json + bge-reranker tokenizer.json ship without an explicit pad_token
        # mapping in the slow-tokenizer fallback path. Try to find an existing special token
        # already in vocab, else add [PAD]. add_special_tokens is the only reliable mutation path
        # for fast tokenizers in transformers >= 5.
        if self.tokenizer.pad_token is None or self.tokenizer.pad_token_id is None:
            for cand in (self.tokenizer.eos_token, self.tokenizer.sep_token, self.tokenizer.unk_token):
                if cand:
                    self.tokenizer.add_special_tokens({"pad_token": cand})
                    break
            else:
                self.tokenizer.add_special_tokens({"pad_token": "[PAD]"})
        print(f"[rerank] loaded model + tokenizer in {time.time()-t0:.2f}s expected_inputs={sorted(self.expected_inputs)}", file=sys.stderr)

    def rerank(self, query, candidates, top_k):
        if not candidates:
            return []
        enc = self.tokenizer(
            [query] * len(candidates),
            [text for _, text in candidates],
            padding=True, truncation=True, max_length=512, return_tensors="np",
            return_token_type_ids=True,
        )
        # Feed only the inputs the model expects (bge-reranker-base: 2 inputs; MiniLM: 3 inputs)
        inputs = {
            "input_ids": enc["input_ids"].astype("int64"),
            "attention_mask": enc["attention_mask"].astype("int64"),
        }
        if "token_type_ids" in self.expected_inputs:
            if "token_type_ids" in enc:
                inputs["token_type_ids"] = enc["token_type_ids"].astype("int64")
            else:
                import numpy as _np
                inputs["token_type_ids"] = _np.zeros_like(enc["input_ids"], dtype=_np.int64)
        logits = self.session.run(None, inputs)[0]
        scores = logits.squeeze(-1).tolist() if logits.ndim > 1 else logits.tolist()
        ranked = sorted(zip([sid for sid, _ in candidates], scores), key=lambda x: -x[1])
        return [s for s, _ in ranked[:top_k]]


def score_ranking(ranking, gold):
    r1 = 1 if ranking[:1] and ranking[0] in gold else 0
    r5 = 1 if any(s in gold for s in ranking[:5]) else 0
    r10 = 1 if any(s in gold for s in ranking[:10]) else 0
    mrr = 0.0
    for k, s in enumerate(ranking[:10], start=1):
        if s in gold:
            mrr = 1.0 / k
            break
    return r1, r5, r10, mrr


def evaluate(corpus_path, subset, reranker_model, reranker_tok, out_path):
    rr = Reranker(reranker_model, reranker_tok)
    overall = {"r_at_1": 0, "r_at_5": 0, "r_at_10": 0, "mrr": 0.0, "n": 0}
    rerank_total = {"r_at_1": 0, "r_at_5": 0, "r_at_10": 0, "mrr": 0.0, "n": 0}
    per_instance = []
    with open(corpus_path) as f:
        for i, line in enumerate(f):
            if i >= subset:
                break
            inst = json.loads(line)
            qtext = inst["question"]
            gold = set(inst.get("answer_session_ids", []))
            sids = inst.get("haystack_session_ids", [])
            sessions = inst.get("haystack_sessions", [])
            if not sids or not sessions:
                continue
            qv = embed(qtext)
            sid_vecs = []
            docs = []
            for sid, turns in zip(sids, sessions):
                text = truncate_bytes(" ".join(t.get("content", "") for t in turns))
                if not text:
                    continue
                sid_vecs.append((sid, embed(text)))
                docs.append((sid, text))
            vec_top = vector_rank(qv, sid_vecs)
            bm_top = bm25_rank(qtext, docs)
            fused = rrf_fuse([vec_top, bm_top])
            text_by_sid = {sid: text for sid, text in docs}
            cand_pairs = [(sid, text_by_sid.get(sid, "")) for sid in fused if sid in text_by_sid]
            reranked = rr.rerank(qtext, cand_pairs, TOP_K_FINAL)
            r1, r5, r10, mrr = score_ranking(fused, gold)
            rr1, rr5, rr10, rmrr = score_ranking(reranked, gold)
            overall["r_at_1"] += r1
            overall["r_at_5"] += r5
            overall["r_at_10"] += r10
            overall["mrr"] += mrr
            overall["n"] += 1
            rerank_total["r_at_1"] += rr1
            rerank_total["r_at_5"] += rr5
            rerank_total["r_at_10"] += rr10
            rerank_total["mrr"] += rmrr
            rerank_total["n"] += 1
            per_instance.append({
                "q_id": inst.get("question_id"),
                "question": qtext[:120],
                "gold": list(gold),
                "fused_top5": fused[:5],
                "rerank_top5": reranked[:5],
                "rrf_score": {"r1": r1, "r5": r5, "r10": r10, "mrr": mrr},
                "rerank_score": {"r1": rr1, "r5": rr5, "r10": rr10, "mrr": rmrr},
            })
            print(f"[{i+1}/{subset}] q={qtext[:50]} rrf_r5={r5} rerank_r5={rr5}", file=sys.stderr)
    n = overall["n"] or 1

    def norm(o):
        return {"r_at_1": o["r_at_1"] / n, "r_at_5": o["r_at_5"] / n,
                "r_at_10": o["r_at_10"] / n, "mrr": o["mrr"] / n, "n": n}

    result = {
        "ts": time.strftime("%Y%m%dT%H%M%SZ", time.gmtime()),
        "corpus": str(corpus_path),
        "subset": subset,
        "reranker_model": reranker_model,
        "rrf_only": norm(overall),
        "rrf_plus_rerank": norm(rerank_total),
        "delta_r_at_5": (rerank_total["r_at_5"] - overall["r_at_5"]) / n,
        "per_instance": per_instance,
    }
    Path(out_path).write_text(json.dumps(result, indent=2))
    print(json.dumps({"rrf_only": result["rrf_only"],
                      "rrf_plus_rerank": result["rrf_plus_rerank"],
                      "delta_r_at_5": result["delta_r_at_5"]}, indent=2))


if __name__ == "__main__":
    ap = argparse.ArgumentParser()
    ap.add_argument("--corpus", required=True, type=Path)
    ap.add_argument("--subset", type=int, default=20)
    ap.add_argument("--out", required=True, type=Path)
    ap.add_argument("--reranker-model",
                    default=str(Path.home() / ".cache/amore/models/ms-marco-MiniLM-L-6-v2/model.onnx"))
    ap.add_argument("--reranker-tokenizer",
                    default=str(Path.home() / ".cache/amore/models/ms-marco-MiniLM-L-6-v2/tokenizer.json"))
    args = ap.parse_args()
    evaluate(args.corpus, args.subset, args.reranker_model, args.reranker_tokenizer, args.out)
