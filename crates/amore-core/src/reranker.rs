// reranker.rs — cross-encoder reranker (H.3)
//
// Runs BAAI/bge-reranker-base ONNX inference via ort + tokenizers.
// Takes top-50 RRF-fused candidates from recall.rs and reranks to top-K.
//
// Model loading is deferred to runtime — the ONNX file is NOT bundled.
// See docs/H3-RERANKER-NDCG-PLAN.md for download instructions and
// nDCG@10 comparison procedure.

use std::path::{Path, PathBuf};

use anyhow::{anyhow, Context, Result};
use ort::session::Session;
use ort::value::Tensor;
use tokenizers::Tokenizer;
use tracing::debug;

/// Maximum sequence length for cross-encoder input (standard BERT limit).
const DEFAULT_MAX_LENGTH: usize = 512;

/// Cross-encoder reranker backed by BAAI/bge-reranker-base ONNX.
///
/// # Usage
/// ```no_run
/// use amore_core::reranker::Reranker;
/// # fn main() -> anyhow::Result<()> {
/// let reranker = Reranker::from_default_paths()?;
/// let hits = reranker.rerank(
///     "rust async runtime",
///     vec![(1, "Tokio is a Rust async runtime".to_string())],
///     10,
/// )?;
/// # Ok(())
/// # }
/// ```
pub struct Reranker {
    session: Session,
    tokenizer: Tokenizer,
    max_length: usize,
}

impl Reranker {
    /// Load reranker from explicit model and tokenizer paths.
    ///
    /// `model_path` — path to bge-reranker-base.onnx (export via optimum-cli if needed)
    /// `tokenizer_path` — path to tokenizer.json (from HuggingFace hub)
    pub fn new(model_path: &Path, tokenizer_path: &Path) -> Result<Self> {
        Self::new_with_max_length(model_path, tokenizer_path, DEFAULT_MAX_LENGTH)
    }

    /// Load reranker with explicit max sequence length.
    pub fn new_with_max_length(
        model_path: &Path,
        tokenizer_path: &Path,
        max_length: usize,
    ) -> Result<Self> {
        if !model_path.exists() {
            return Err(anyhow!(
                "ONNX model not found at {}: download with `huggingface-cli download BAAI/bge-reranker-base`",
                model_path.display()
            ));
        }
        if !tokenizer_path.exists() {
            return Err(anyhow!(
                "tokenizer.json not found at {}",
                tokenizer_path.display()
            ));
        }

        let session = Session::builder()
            .context("failed to create ORT session builder")?
            .commit_from_file(model_path)
            .with_context(|| format!("failed to load ONNX model from {}", model_path.display()))?;

        let tokenizer = Tokenizer::from_file(tokenizer_path)
            .map_err(|e| anyhow!("failed to load tokenizer from {}: {e}", tokenizer_path.display()))?;

        Ok(Self { session, tokenizer, max_length })
    }

    /// Load from the default amore data directory.
    ///
    /// Looks for:
    ///   `<data_local_dir>/Amore/models/bge-reranker-base.onnx`
    ///   `<data_local_dir>/Amore/models/tokenizer.json`
    ///
    /// Download:
    ///   `huggingface-cli download BAAI/bge-reranker-base --local-dir <data_local_dir>/Amore/models/`
    ///   Then export: `optimum-cli export onnx --model BAAI/bge-reranker-base bge-reranker-base/`
    pub fn from_default_paths() -> Result<Self> {
        let base = default_model_dir()?;
        let model_path = base.join("bge-reranker-base.onnx");
        let tokenizer_path = base.join("tokenizer.json");
        Self::new(&model_path, &tokenizer_path)
    }

    /// Rerank candidates using cross-encoder scoring.
    ///
    /// Each `(id, text)` pair is encoded as `[CLS] query [SEP] text [SEP]`,
    /// passed through the cross-encoder, and the logit[0] is used as the score.
    /// Returns top-`top_k` entries sorted by score descending.
    ///
    /// Requires `&mut self` because `ort::Session::run` takes `&mut self`.
    pub fn rerank(
        &mut self,
        query: &str,
        candidates: Vec<(u64, String)>,
        top_k: usize,
    ) -> Result<Vec<(u64, f32)>> {
        if candidates.is_empty() {
            return Ok(Vec::new());
        }

        let n = candidates.len();
        debug!("reranker: scoring {n} candidates for query {:?}", query);

        // Tokenize all (query, doc) pairs — produces [CLS] q [SEP] d [SEP]
        let pairs: Vec<tokenizers::EncodeInput<'_>> = candidates
            .iter()
            .map(|(_, text)| {
                tokenizers::EncodeInput::Dual(
                    tokenizers::InputSequence::Raw(std::borrow::Cow::Borrowed(query)),
                    tokenizers::InputSequence::Raw(std::borrow::Cow::Borrowed(text.as_str())),
                )
            })
            .collect();

        let encodings = self
            .tokenizer
            .encode_batch(pairs, true)
            .map_err(|e| anyhow!("tokenizer batch encode failed: {e}"))?;

        // Max length across this batch, capped at self.max_length
        let seq_len = encodings
            .iter()
            .map(|e| e.get_ids().len())
            .max()
            .unwrap_or(0)
            .min(self.max_length);

        if seq_len == 0 {
            return Ok(Vec::new());
        }

        // Build flat i64 tensors: [n, seq_len]
        let mut input_ids_flat: Vec<i64> = Vec::with_capacity(n * seq_len);
        let mut attention_mask_flat: Vec<i64> = Vec::with_capacity(n * seq_len);
        let mut token_type_ids_flat: Vec<i64> = Vec::with_capacity(n * seq_len);

        for enc in &encodings {
            let ids = enc.get_ids();
            let mask = enc.get_attention_mask();
            let type_ids = enc.get_type_ids();
            let actual = ids.len().min(seq_len);

            for i in 0..seq_len {
                if i < actual {
                    input_ids_flat.push(ids[i] as i64);
                    attention_mask_flat.push(mask[i] as i64);
                    token_type_ids_flat.push(type_ids[i] as i64);
                } else {
                    // pad with 0
                    input_ids_flat.push(0);
                    attention_mask_flat.push(0);
                    token_type_ids_flat.push(0);
                }
            }
        }

        // ort ToShape is implemented for [usize; N] and Vec<usize>, not (usize, usize).
        // Shape is consumed by from_array so we create it fresh each time.
        let input_ids = Tensor::<i64>::from_array(([n, seq_len], input_ids_flat))
            .context("failed to create input_ids tensor")?;
        let attention_mask = Tensor::<i64>::from_array(([n, seq_len], attention_mask_flat))
            .context("failed to create attention_mask tensor")?;
        let token_type_ids = Tensor::<i64>::from_array(([n, seq_len], token_type_ids_flat))
            .context("failed to create token_type_ids tensor")?;

        let outputs = self
            .session
            .run(ort::inputs![
                "input_ids" => input_ids,
                "attention_mask" => attention_mask,
                "token_type_ids" => token_type_ids,
            ])
            .context("ORT session run failed")?;

        // bge-reranker-base outputs logits [n, 1]
        let (_, logits_view) = outputs["logits"]
            .try_extract_tensor::<f32>()
            .context("failed to extract logits from ORT output")?;

        let logits: Vec<f32> = logits_view.to_vec();

        // Pair scores with original candidate IDs
        let mut scored: Vec<(u64, f32)> = candidates
            .into_iter()
            .zip(logits.iter().copied())
            .map(|((id, _), score)| (id, score))
            .collect();

        // Stable sort descending by score
        scored.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
        scored.truncate(top_k);

        debug!(
            "reranker: top score={:.4}, bottom score={:.4}",
            scored.first().map(|(_, s)| *s).unwrap_or(0.0),
            scored.last().map(|(_, s)| *s).unwrap_or(0.0)
        );

        Ok(scored)
    }
}

/// Returns the default amore models directory (`<data_local_dir>/Amore/models/`).
fn default_model_dir() -> Result<PathBuf> {
    let base = dirs::data_local_dir()
        .ok_or_else(|| anyhow!("could not resolve data_local_dir; set XDG_DATA_HOME or LOCALAPPDATA"))?;
    Ok(base.join("Amore").join("models"))
}
