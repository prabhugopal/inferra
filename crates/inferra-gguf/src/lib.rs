use std::{fs::File, io::Read, path::Path};

use anyhow::{bail, Context, Result};
use candle_core::quantized::gguf_file::{Content, Value, VersionedMagic};
use candle_core::{DType, Device};
use nalgebra::{DMatrix, SymmetricEigen};
use serde::Serialize;

const GGUF_MAGIC: [u8; 4] = *b"GGUF";

#[derive(Debug, PartialEq, Eq)]
pub struct GgufHeader {
    pub version: u32,
    pub tensor_count: u64,
    pub metadata_kv_count: u64,
}

pub fn read_header(path: &Path) -> Result<GgufHeader> {
    let mut file = File::open(path)
        .with_context(|| format!("cannot open {}", path.display()))?;
    read_header_from(&mut file)
}

fn read_header_from(reader: &mut impl Read) -> Result<GgufHeader> {
    let mut magic = [0_u8; 4];
    reader.read_exact(&mut magic).context("truncated GGUF magic")?;
    if magic != GGUF_MAGIC {
        bail!("not a GGUF file: expected GGUF magic bytes");
    }

    let version = read_u32_le(reader).context("truncated GGUF version")?;
    if !(2..=3).contains(&version) {
        bail!("unsupported GGUF version {version}; inferra supports v2 and v3");
    }

    Ok(GgufHeader {
        version,
        tensor_count: read_u64_le(reader).context("truncated GGUF tensor count")?,
        metadata_kv_count: read_u64_le(reader).context("truncated GGUF metadata count")?,
    })
}

fn read_u32_le(reader: &mut impl Read) -> Result<u32> {
    let mut bytes = [0_u8; 4];
    reader.read_exact(&mut bytes)?;
    Ok(u32::from_le_bytes(bytes))
}

fn read_u64_le(reader: &mut impl Read) -> Result<u64> {
    let mut bytes = [0_u8; 8];
    reader.read_exact(&mut bytes)?;
    Ok(u64::from_le_bytes(bytes))
}

// --- Full composition: what the file is actually made of ---
//
// Backed by `candle_core::quantized::gguf_file` — the same reader mistral.rs
// itself uses to load GGUF models — rather than a second hand-rolled parser.
// The 24-byte header above stays as-is for the CLI's quick check; this is
// the richer read for the desktop app's Model Inspector.

const HEADER_SIZE: u64 = 24;

#[derive(Debug, Clone, Serialize)]
pub struct MetadataEntry {
    pub key: String,
    pub value: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct TensorSummary {
    pub name: String,
    pub shape: Vec<usize>,
    pub dtype: String,
    pub byte_size: u64,
}

/// A GGUF file's layout and contents, in one read: the byte regions that
/// make it up (fixed header, metadata + tensor descriptors, tensor data),
/// every metadata key/value, and every tensor's shape/dtype/size — the
/// "what composes this file, and how is it bundled" view.
#[derive(Debug, Clone, Serialize)]
pub struct GgufComposition {
    pub version: u32,
    pub file_size: u64,
    pub header_size: u64,
    /// Typed metadata entries plus tensor name/shape/dtype descriptors —
    /// GGUF stores these back-to-back with no separate boundary, so this is
    /// reported as one region rather than two.
    pub metadata_and_descriptors_size: u64,
    pub tensor_data_offset: u64,
    pub tensor_data_size: u64,
    pub metadata: Vec<MetadataEntry>,
    /// Sorted largest first — the tensors that actually dominate the file
    /// (usually the token embedding and output projection) surface first.
    pub tensors: Vec<TensorSummary>,
    /// The raw Jinja chat template (`tokenizer.chat_template`), if the file
    /// has one — also present as a regular entry in `metadata`, but that
    /// list isn't the place to read a multi-hundred-character template.
    pub chat_template: Option<String>,
    /// BOS/EOS/UNK/PAD resolved to their actual decoded text — GGUF only
    /// stores these as numeric vocabulary indices otherwise.
    pub special_tokens: Vec<SpecialToken>,
    /// `None` when the file's architecture metadata doesn't have the
    /// fields this needs (e.g. a non-attention architecture) — omitted
    /// rather than guessed.
    pub kv_cache: Option<KvCacheEstimate>,
}

#[derive(Debug, Clone, Serialize)]
pub struct SpecialToken {
    pub label: String,
    pub text: String,
}

/// The KV cache's size is architecture, not weights — computable from
/// metadata alone, without loading a single tensor. Assumes an F16 cache
/// (candle/mistral.rs's default); an actual running backend may choose
/// differently (quantized KV, etc.), so this is a reference figure, not a
/// claim about what any particular running process is doing right now.
#[derive(Debug, Clone, Serialize)]
pub struct KvCacheEstimate {
    pub bytes_per_token: u64,
    pub context_length: Option<u64>,
    pub bytes_at_max_context: Option<u64>,
}

pub fn read_composition(path: &Path) -> Result<GgufComposition> {
    let mut file = File::open(path).with_context(|| format!("cannot open {}", path.display()))?;
    let file_size = file.metadata().with_context(|| format!("cannot stat {}", path.display()))?.len();
    let content = Content::read(&mut file)
        .map_err(|error| anyhow::anyhow!("failed to parse {}: {error}", path.display()))?;

    let version = match content.magic {
        VersionedMagic::GgufV1 => 1,
        VersionedMagic::GgufV2 => 2,
        VersionedMagic::GgufV3 => 3,
    };

    let mut metadata: Vec<MetadataEntry> = content
        .metadata
        .iter()
        .map(|(key, value)| MetadataEntry { key: key.clone(), value: describe_value(value) })
        .collect();
    metadata.sort_by(|a, b| a.key.cmp(&b.key));

    let mut tensors: Vec<TensorSummary> = content
        .tensor_infos
        .iter()
        .map(|(name, info)| TensorSummary {
            name: name.clone(),
            shape: info.shape.dims().to_vec(),
            dtype: format!("{:?}", info.ggml_dtype),
            byte_size: tensor_byte_size(info),
        })
        .collect();
    tensors.sort_by_key(|t| std::cmp::Reverse(t.byte_size));

    let tensor_data_offset = content.tensor_data_offset;

    let chat_template =
        content.metadata.get("tokenizer.chat_template").and_then(|v| v.to_string().ok()).cloned();
    let special_tokens = resolve_special_tokens(&content);
    let kv_cache = estimate_kv_cache(&content);

    Ok(GgufComposition {
        version,
        file_size,
        header_size: HEADER_SIZE,
        metadata_and_descriptors_size: tensor_data_offset.saturating_sub(HEADER_SIZE),
        tensor_data_offset,
        tensor_data_size: file_size.saturating_sub(tensor_data_offset),
        metadata,
        tensors,
        chat_template,
        special_tokens,
        kv_cache,
    })
}

/// GGUF namespaces per-architecture fields under `general.architecture`'s
/// own value (e.g. `llama.attention.head_count`, `qwen2.attention.head_count`)
/// — not a fixed `"llama."` prefix — so the key has to be built from the
/// file's own declared architecture to work across model families.
fn estimate_kv_cache(content: &Content) -> Option<KvCacheEstimate> {
    let arch = content.metadata.get("general.architecture")?.to_string().ok()?;
    let field = |name: &str| content.metadata.get(&format!("{arch}.{name}"))?.to_u32().ok();

    let block_count = field("block_count")? as u64;
    let head_count = field("attention.head_count")? as u64;
    let embedding_length = field("embedding_length")? as u64;
    // Absent entirely (no grouped-query attention declared) means every
    // head keeps its own KV — the same as head_count, not zero.
    let head_count_kv = field("attention.head_count_kv").map(u64::from).unwrap_or(head_count);
    let head_dim = embedding_length.checked_div(head_count.max(1))?;

    const F16_BYTES: u64 = 2;
    const K_AND_V: u64 = 2;
    let bytes_per_token = K_AND_V * block_count * head_count_kv * head_dim * F16_BYTES;

    let context_length = field("context_length").map(u64::from);
    let bytes_at_max_context = context_length.map(|ctx| bytes_per_token * ctx);

    Some(KvCacheEstimate { bytes_per_token, context_length, bytes_at_max_context })
}

/// The tokenizer family declared by the file (`"gpt2"`, `"llama"`, ...) —
/// determines which raw-vocabulary decoding convention applies.
fn tokenizer_kind(content: &Content) -> Option<String> {
    content.metadata.get("tokenizer.ggml.model").and_then(|v| v.to_string().ok()).cloned()
}

/// GGUF stores the tokenizer's raw vocabulary, not display-ready text.
/// Byte-level BPE (gpt2) encodes every byte as a printable character so the
/// vocab is safely representable as JSON strings — a leading space becomes
/// 'Ġ' (U+0120), not an actual space — while SentencePiece (llama) just
/// uses '▁' as a plain word-boundary marker. Decode either back to normal
/// text so labels (and resolved special tokens) read as real words.
fn decode_vocab_token(raw: &str, kind: Option<&str>, reverse: &std::collections::HashMap<char, u8>) -> String {
    match kind {
        Some("gpt2") => decode_gpt2_bpe_token(raw, reverse),
        Some("llama") => raw.replace('\u{2581}', " "),
        _ => raw.to_string(),
    }
}

/// Looks up one vocabulary entry by ID and decodes it — without touching
/// the rest of the (possibly 50k+ entry) vocabulary array, unlike
/// [`read_token_strings`], which decodes all of it for the embedding view.
fn resolve_special_token(
    content: &Content,
    id_key: &str,
    kind: Option<&str>,
    reverse: &std::collections::HashMap<char, u8>,
) -> Option<String> {
    let id = content.metadata.get(id_key)?.to_u32().ok()? as usize;
    let items = content.metadata.get("tokenizer.ggml.tokens")?.to_vec().ok()?;
    let raw = items.get(id)?.to_string().ok()?;
    Some(decode_vocab_token(raw, kind, reverse))
}

fn resolve_special_tokens(content: &Content) -> Vec<SpecialToken> {
    let kind = tokenizer_kind(content);
    let reverse = gpt2_unicode_to_byte();
    [
        ("BOS", "tokenizer.ggml.bos_token_id"),
        ("EOS", "tokenizer.ggml.eos_token_id"),
        ("UNK", "tokenizer.ggml.unknown_token_id"),
        ("PAD", "tokenizer.ggml.padding_token_id"),
    ]
    .into_iter()
    .filter_map(|(label, id_key)| {
        resolve_special_token(content, id_key, kind.as_deref(), &reverse)
            .map(|text| SpecialToken { label: label.to_string(), text })
    })
    .collect()
}

fn tensor_byte_size(info: &candle_core::quantized::gguf_file::TensorInfo) -> u64 {
    let elems = info.shape.elem_count() as u64;
    let block_size = info.ggml_dtype.block_size() as u64;
    let type_size = info.ggml_dtype.type_size() as u64;
    elems / block_size.max(1) * type_size
}

fn describe_value(value: &Value) -> String {
    match value {
        Value::U8(v) => v.to_string(),
        Value::I8(v) => v.to_string(),
        Value::U16(v) => v.to_string(),
        Value::I16(v) => v.to_string(),
        Value::U32(v) => v.to_string(),
        Value::I32(v) => v.to_string(),
        Value::U64(v) => v.to_string(),
        Value::I64(v) => v.to_string(),
        Value::F32(v) => v.to_string(),
        Value::F64(v) => v.to_string(),
        Value::Bool(v) => v.to_string(),
        Value::String(v) => v.clone(),
        // A big array (the vocabulary, merge rules, ...) is summarized —
        // showing it inline would dwarf the rest of the table. A short one
        // (languages, tags, ...) is genuinely more useful shown in full.
        Value::Array(items) if items.len() <= 20 => {
            items.iter().map(describe_value).collect::<Vec<_>>().join(", ")
        }
        Value::Array(items) => format!("[{} items]", items.len()),
    }
}

// --- Embedding-space sampling: where do words cluster? ---

#[derive(Debug, Clone, Serialize)]
pub struct EmbeddingPoint {
    pub token: String,
    pub x: f32,
    pub y: f32,
    pub z: f32,
    /// Whether this point is one of the caller's seed tokens (from real
    /// text) rather than a neighbor found around it — lets the UI draw the
    /// two apart instead of showing an undifferentiated cloud.
    pub is_seed: bool,
}

struct EmbeddingMatrix {
    rows: Vec<Vec<f32>>,
    tokens: Vec<String>,
    vocab_size: usize,
    hidden_dim: usize,
}

fn load_embedding_matrix(path: &Path, tensor_name: &str) -> Result<EmbeddingMatrix> {
    let mut file = File::open(path).with_context(|| format!("cannot open {}", path.display()))?;
    let content = Content::read(&mut file)
        .map_err(|error| anyhow::anyhow!("failed to parse {}: {error}", path.display()))?;

    let device = Device::Cpu;
    let qtensor = content
        .tensor(&mut file, tensor_name, &device)
        .with_context(|| format!("no tensor named '{tensor_name}' (try 'token_embd.weight')"))?;
    let tensor = qtensor.dequantize(&device).context("failed to dequantize embedding tensor")?;
    let tensor = tensor.to_dtype(DType::F32).context("failed to convert embedding tensor to f32")?;
    let (vocab_size, hidden_dim) = tensor
        .dims2()
        .with_context(|| format!("tensor '{tensor_name}' is not 2-dimensional (a [vocab, hidden] embedding matrix)"))?;
    let rows: Vec<Vec<f32>> = tensor.to_vec2().context("failed to read embedding tensor values")?;
    let tokens = read_token_strings(&content, vocab_size);

    Ok(EmbeddingMatrix { rows, tokens, vocab_size, hidden_dim })
}

fn points_from_indices(
    matrix: &EmbeddingMatrix,
    indices: &[usize],
    seeds: &std::collections::HashSet<usize>,
) -> Result<Vec<EmbeddingPoint>> {
    let sampled: Vec<&Vec<f32>> = indices.iter().map(|&i| &matrix.rows[i]).collect();
    let coords = pca_3d(&sampled, matrix.hidden_dim)?;

    Ok(indices
        .iter()
        .zip(coords)
        .map(|(&i, [x, y, z])| EmbeddingPoint {
            token: matrix.tokens.get(i).cloned().unwrap_or_else(|| format!("<token {i}>")),
            x,
            y,
            z,
            is_seed: seeds.contains(&i),
        })
        .collect())
}

/// Dequantizes `tensor_name` (typically `token_embd.weight`), samples up to
/// `sample_size` rows evenly across the vocabulary, and projects them to 3
/// dimensions via PCA — so real embedding-space clustering becomes visible
/// without shipping a full 30k+-row matrix to the UI. Token labels come
/// from the file's own `tokenizer.ggml.tokens` metadata, not guessed.
pub fn sample_embeddings(path: &Path, tensor_name: &str, sample_size: usize) -> Result<Vec<EmbeddingPoint>> {
    let matrix = load_embedding_matrix(path, tensor_name)?;

    let sample_size = sample_size.clamp(1, matrix.vocab_size);
    let stride = matrix.vocab_size as f64 / sample_size as f64;
    let indices: Vec<usize> = (0..sample_size)
        .map(|i| (((i as f64) * stride) as usize).min(matrix.vocab_size - 1))
        .collect();

    points_from_indices(&matrix, &indices, &std::collections::HashSet::new())
}

/// Like [`sample_embeddings`], but centered on real text instead of an even
/// spread: `seed_token_ids` (from the model's own tokenizer — see
/// `InferenceBackend::tokenize`) are always included, and the remaining
/// budget up to `total_sample_size` is filled with their nearest neighbors
/// by cosine similarity, computed against the full vocabulary. So instead of
/// a random cross-section of the embedding space, this shows what's
/// actually clustered around the words you typed.
pub fn sample_embeddings_near(
    path: &Path,
    tensor_name: &str,
    seed_token_ids: &[u32],
    total_sample_size: usize,
) -> Result<Vec<EmbeddingPoint>> {
    let matrix = load_embedding_matrix(path, tensor_name)?;

    let seed_indices: Vec<usize> =
        seed_token_ids.iter().map(|&id| id as usize).filter(|&i| i < matrix.vocab_size).collect();
    if seed_indices.is_empty() {
        bail!("no valid seed token ids for a vocabulary of {}", matrix.vocab_size);
    }
    let seeds: std::collections::HashSet<usize> = seed_indices.iter().copied().collect();
    let seed_rows: Vec<&Vec<f32>> = seed_indices.iter().map(|&i| &matrix.rows[i]).collect();

    let mut ranked: Vec<(usize, f32)> = (0..matrix.vocab_size)
        .filter(|i| !seeds.contains(i))
        .map(|i| {
            let row = &matrix.rows[i];
            let best = seed_rows.iter().map(|seed| cosine_similarity(seed, row)).fold(f32::MIN, f32::max);
            (i, best)
        })
        .collect();
    ranked.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));

    let neighbor_budget = total_sample_size.saturating_sub(seed_indices.len());
    let mut indices = seed_indices;
    indices.extend(ranked.into_iter().take(neighbor_budget).map(|(i, _)| i));

    points_from_indices(&matrix, &indices, &seeds)
}

fn cosine_similarity(a: &[f32], b: &[f32]) -> f32 {
    let dot: f32 = a.iter().zip(b).map(|(x, y)| x * y).sum();
    let norm_a = a.iter().map(|x| x * x).sum::<f32>().sqrt();
    let norm_b = b.iter().map(|x| x * x).sum::<f32>().sqrt();
    if norm_a == 0.0 || norm_b == 0.0 { 0.0 } else { dot / (norm_a * norm_b) }
}

/// `tokenizer.ggml.tokens` is the GGUF-standard vocabulary array — reading
/// it directly means every label shown is the model's own token text, not
/// a re-tokenization or approximation.
fn read_token_strings(content: &Content, expected: usize) -> Vec<String> {
    let Some(value) = content.metadata.get("tokenizer.ggml.tokens") else {
        return Vec::new();
    };
    let Ok(items) = value.to_vec() else {
        return Vec::new();
    };
    let raw: Vec<String> = items
        .iter()
        .take(expected)
        .map(|item| item.to_string().cloned().unwrap_or_else(|_| "?".to_string()))
        .collect();

    // GGUF stores the tokenizer's raw vocabulary, not display-ready text —
    // see `decode_vocab_token` for what that means and why.
    let kind = tokenizer_kind(content);
    let reverse = gpt2_unicode_to_byte();
    raw.into_iter().map(|s| decode_vocab_token(&s, kind.as_deref(), &reverse)).collect()
}

/// GPT-2's canonical byte<->unicode mapping (the same algorithm every
/// GPT-2-family BPE tokenizer uses): printable ASCII/Latin-1 bytes map to
/// themselves, and the remaining bytes (space, control characters, etc.)
/// map into a private range starting at U+0100 — byte 0x20 (space) lands
/// on U+0120 ('Ġ'), which is why raw vocab entries show that character
/// instead of a real space.
fn gpt2_byte_to_unicode() -> [char; 256] {
    let mut printable = [false; 256];
    for b in b'!'..=b'~' {
        printable[b as usize] = true;
    }
    for b in 0xA1u32..=0xACu32 {
        printable[b as usize] = true;
    }
    for b in 0xAEu32..=0xFFu32 {
        printable[b as usize] = true;
    }

    let mut table = ['\0'; 256];
    let mut next_private = 0u32;
    for (b, slot) in table.iter_mut().enumerate() {
        *slot = if printable[b] {
            char::from_u32(b as u32).unwrap()
        } else {
            let c = char::from_u32(256 + next_private).unwrap();
            next_private += 1;
            c
        };
    }
    table
}

fn gpt2_unicode_to_byte() -> std::collections::HashMap<char, u8> {
    gpt2_byte_to_unicode().into_iter().enumerate().map(|(b, c)| (c, b as u8)).collect()
}

fn decode_gpt2_bpe_token(raw: &str, reverse: &std::collections::HashMap<char, u8>) -> String {
    let mut bytes = Vec::with_capacity(raw.len());
    for ch in raw.chars() {
        match reverse.get(&ch) {
            Some(&b) => bytes.push(b),
            // Not every vocab entry is byte-mapped (added/special tokens
            // like "<|im_start|>" are often stored literally) — preserve
            // such characters verbatim rather than dropping/corrupting them.
            None => bytes.extend_from_slice(ch.encode_utf8(&mut [0u8; 4]).as_bytes()),
        }
    }
    // A single BPE piece can be a partial multi-byte UTF-8 sequence that
    // only becomes valid once merged with a neighboring piece — fall back
    // to a lossy decode rather than losing the token entirely.
    String::from_utf8(bytes.clone()).unwrap_or_else(|_| String::from_utf8_lossy(&bytes).into_owned())
}

/// A small, transparent PCA: center the sampled rows, eigendecompose their
/// covariance matrix, and project onto the top-3 eigenvectors (by
/// eigenvalue, descending). Deterministic and easy to follow end to end —
/// deliberately not a black-box ML crate for a step this simple.
fn pca_3d(rows: &[&Vec<f32>], dim: usize) -> Result<Vec<[f32; 3]>> {
    let n = rows.len();
    if n == 0 {
        bail!("no rows to reduce");
    }
    let axes = 3.min(dim);

    let mut data = DMatrix::<f64>::zeros(n, dim);
    for (i, row) in rows.iter().enumerate() {
        for (j, &value) in row.iter().enumerate() {
            data[(i, j)] = value as f64;
        }
    }

    let mean = data.row_mean();
    for mut row in data.row_iter_mut() {
        row -= &mean;
    }

    let covariance = (data.transpose() * &data) / ((n.max(2) - 1) as f64);
    let eigen = SymmetricEigen::new(covariance);

    let mut order: Vec<usize> = (0..dim).collect();
    order.sort_by(|&a, &b| {
        eigen.eigenvalues[b]
            .partial_cmp(&eigen.eigenvalues[a])
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    let components: Vec<_> = order.into_iter().take(axes).map(|i| eigen.eigenvectors.column(i).into_owned()).collect();

    Ok(data
        .row_iter()
        .map(|row| {
            let mut coords = [0f32; 3];
            for (axis, component) in components.iter().enumerate() {
                coords[axis] = (row * component)[(0, 0)] as f32;
            }
            coords
        })
        .collect())
}

#[cfg(test)]
mod tests {
    use std::io::Cursor;
    use super::*;

    #[test]
    fn decodes_gpt2_byte_level_bpe_tokens() {
        let reverse = gpt2_unicode_to_byte();
        // 'Ġ' (U+0120) is byte 0x20 (space) under GPT-2's byte-to-unicode
        // mapping — exactly the artifact reported against a real model,
        // confirmed against a real SmolLM2 GGUF (tokenizer.ggml.model = "gpt2").
        assert_eq!(decode_gpt2_bpe_token("ĠFrance", &reverse), " France");
        assert_eq!(decode_gpt2_bpe_token("Ġcap", &reverse), " cap");
        // Printable ASCII maps to itself under GPT-2's table, so ordinary
        // text — including special tokens like "<|im_start|>" — round-trips
        // unchanged either way.
        assert_eq!(decode_gpt2_bpe_token("hello", &reverse), "hello");
        assert_eq!(decode_gpt2_bpe_token("<|im_start|>", &reverse), "<|im_start|>");
        // A character genuinely outside the 256-entry table (defensive
        // path only — a real byte-level BPE vocab never produces one) must
        // still be preserved verbatim, not dropped or corrupted.
        assert_eq!(decode_gpt2_bpe_token("β", &reverse), "β");
    }

    #[test]
    fn reads_a_v3_header() {
        let mut bytes = Vec::new();
        bytes.extend_from_slice(b"GGUF");
        bytes.extend_from_slice(&3_u32.to_le_bytes());
        bytes.extend_from_slice(&42_u64.to_le_bytes());
        bytes.extend_from_slice(&17_u64.to_le_bytes());

        assert_eq!(read_header_from(&mut Cursor::new(bytes)).unwrap(), GgufHeader {
            version: 3,
            tensor_count: 42,
            metadata_kv_count: 17,
        });
    }

    #[test]
    fn rejects_non_gguf_data() {
        assert!(read_header_from(&mut Cursor::new(b"NOPE".to_vec())).is_err());
    }

    fn push_string(bytes: &mut Vec<u8>, s: &str) {
        bytes.extend_from_slice(&(s.len() as u64).to_le_bytes());
        bytes.extend_from_slice(s.as_bytes());
    }

    /// A minimal-but-real GGUF v3 file: one string metadata entry, one F32
    /// tensor — enough for `Content::read` (and so `read_composition`) to
    /// succeed without a full model on disk.
    fn write_minimal_gguf(path: &Path) {
        let mut bytes = Vec::new();
        bytes.extend_from_slice(b"GGUF");
        bytes.extend_from_slice(&3_u32.to_le_bytes());
        bytes.extend_from_slice(&1_u64.to_le_bytes()); // tensor_count
        bytes.extend_from_slice(&1_u64.to_le_bytes()); // metadata_kv_count

        push_string(&mut bytes, "general.architecture");
        bytes.extend_from_slice(&8u32.to_le_bytes()); // TYPE_STRING
        push_string(&mut bytes, "llama");

        push_string(&mut bytes, "token_embd.weight");
        bytes.extend_from_slice(&2u32.to_le_bytes()); // n_dimensions
        bytes.extend_from_slice(&2u64.to_le_bytes());
        bytes.extend_from_slice(&3u64.to_le_bytes());
        bytes.extend_from_slice(&0u32.to_le_bytes()); // GgmlType::F32
        bytes.extend_from_slice(&0u64.to_le_bytes()); // offset

        let header_end = bytes.len() as u64;
        let aligned_start = header_end.div_ceil(32) * 32;
        bytes.resize(aligned_start as usize + 24, 0); // 2*3 f32 = 24 bytes

        std::fs::write(path, bytes).unwrap();
    }

    #[test]
    fn reads_a_full_composition() {
        let path = std::env::temp_dir().join("inferra-gguf-composition-test.gguf");
        write_minimal_gguf(&path);

        let composition = read_composition(&path).unwrap();
        std::fs::remove_file(&path).ok();

        assert_eq!(composition.version, 3);
        assert_eq!(composition.header_size, 24);
        assert_eq!(composition.metadata.len(), 1);
        assert_eq!(composition.metadata[0].key, "general.architecture");
        assert_eq!(composition.metadata[0].value, "llama");
        assert_eq!(composition.tensors.len(), 1);
        assert_eq!(composition.tensors[0].name, "token_embd.weight");
        // GGUF stores dimensions fastest-varying first; candle-core reads
        // them in reverse of that on-disk order, so [2, 3] on disk (as
        // written by write_minimal_gguf below) comes back as [3, 2] here.
        assert_eq!(composition.tensors[0].shape, vec![3, 2]);
        assert_eq!(composition.tensors[0].byte_size, 24);
        assert_eq!(composition.tensor_data_size, 24);
    }

    #[test]
    fn pca_reduces_to_the_requested_number_of_points() {
        let rows: Vec<Vec<f32>> = vec![
            vec![1.0, 0.0, 0.0, 0.0, 0.0],
            vec![0.0, 1.0, 0.0, 0.0, 0.0],
            vec![0.0, 0.0, 1.0, 0.0, 0.0],
            vec![10.0, 10.0, 10.0, 10.0, 10.0],
        ];
        let refs: Vec<&Vec<f32>> = rows.iter().collect();

        let coords = pca_3d(&refs, 5).unwrap();

        assert_eq!(coords.len(), 4);
        for point in &coords {
            assert!(point.iter().all(|c| c.is_finite()));
        }
    }
}
