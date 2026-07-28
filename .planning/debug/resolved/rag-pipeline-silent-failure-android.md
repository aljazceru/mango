---
status: resolved
trigger: "RAG documents are ingested but responses show no awareness of document content — silent failure, no errors"
created: 2026-03-26T00:00:00Z
updated: 2026-03-26T00:00:00Z
---

## Current Focus
<!-- OVERWRITE on each update - reflects NOW -->

hypothesis: CONFIRMED — Android EmbeddingProvider stub returns emptyList() instead of zero-vectors, breaking both ingestion and retrieval
test: Root cause verified by tracing emptyList() through EmbeddingComplete handler and query_emb guard
expecting: Fix: change Android stub to return List<Float> of size texts.size * 384 all-zeros (matching NullEmbeddingProvider)
next_action: Apply fix to AppManager.kt

## Symptoms
<!-- Written during gathering, then IMMUTABLE -->

expected: Vector index search returns relevant chunks; those chunks are injected into chat context before the LLM call
actual: Documents are attached/ingested, user asks a question about the doc, LLM response ignores document content entirely
errors: No errors — silent failure. Everything appears to work (ingestion succeeds, no crashes) but content is never used
reproduction: Ingest a document via Android UI, send a message asking about its content, response has no awareness of the doc
started: Uncertain — want to verify from scratch whether this has ever worked
platform: Android (Jetpack Compose)

## Eliminated
<!-- APPEND only - prevents re-investigating -->

- hypothesis: Vector index path mismatch (index written to different dir than read from)
  evidence: VectorIndex.new() uses single data_dir for both read and write; both use same path format. Same data_dir passed at init.
  timestamp: 2026-03-26

- hypothesis: AttachDocumentToConversation not wired in Android UI
  evidence: MainApp.kt line 41-42 correctly dispatches AttachDocumentToConversation and DetachDocumentFromConversation. ChatScreen DocAttachSheet correctly fires callbacks.
  timestamp: 2026-03-26

- hypothesis: Context injection code skips retrieval even when docs are attached
  evidence: lib.rs line 984-1016 correctly guards on current_conversation_attached_docs.is_empty() and injects context when chunks are found. Logic is correct.
  timestamp: 2026-03-26

- hypothesis: Retrieval returns 0 results due to score threshold
  evidence: VectorIndex.search() has no threshold filtering — returns raw top_k results by distance. No threshold applied in lib.rs either.
  timestamp: 2026-03-26

## Evidence
<!-- APPEND only - facts discovered -->

- timestamp: 2026-03-26
  checked: AppManager.kt lines 85-89
  found: Android EmbeddingProvider stub always returns emptyList() (truly empty, zero elements)
  implication: embed() contract requires Vec<f32> of length texts.len() * EMBEDDING_DIM (384). Returning empty breaks both ingestion and retrieval.

- timestamp: 2026-03-26
  checked: lib.rs EmbeddingComplete handler (lines 3382-3388)
  found: Handler iterates chunk_rowids and checks `if end <= embeddings.len()`. With empty embeddings (len=0), end = i*384+384 > 0, so the guard is never satisfied. No vectors added to HNSW index. index.save() is still called because chunk_rowids is non-empty (from chunking stage). Toast "Document indexed successfully" fires regardless.
  implication: Ingestion appears to succeed (toast shown, progress cleared) but HNSW index contains 0 vectors.

- timestamp: 2026-03-26
  checked: lib.rs RAG context injection (lines 984-1018)
  found: query_emb = embedding_provider.embed([text]) returns empty vec. Guard at line 987 `if !query_emb.is_empty()` is false. Falls through to base_system_prompt. No retrieval attempted.
  implication: Even if the index had been populated somehow, retrieval would still fail because the query embedding is empty.

- timestamp: 2026-03-26
  checked: embedding/mod.rs NullEmbeddingProvider
  found: NullEmbeddingProvider.embed() returns vec![0.0f32; texts.len() * EMBEDDING_DIM] — correct size, zero values. This is the correct stub contract.
  implication: The Android stub should match this contract: return EMBEDDING_DIM * texts.size zero floats per text.

- timestamp: 2026-03-26
  checked: DocumentLibraryScreen.kt line 56 comment
  found: "Phase 8: Replace with CoreML/XNNPACK EmbeddingProvider when custom ORT build is ready"
  implication: The empty stub was intentional as a temporary placeholder, but the wrong return type was used. Should have matched NullEmbeddingProvider behavior (zero-vectors) not truly empty.

## Resolution
<!-- OVERWRITE as understanding evolves -->

root_cause: AppManager.kt Android EmbeddingProvider stub returns emptyList() instead of a correctly-sized zero-vector list. The EmbeddingProvider contract requires texts.size * 384 Float values. emptyList() causes: (1) ingestion silently drops all vectors from the HNSW index because the EmbeddingComplete handler's bounds check always fails, and (2) retrieval is skipped entirely because the query embedding guard `if !query_emb.is_empty()` is false. The "Document indexed successfully" toast fires regardless (it checks chunk_rowids.is_empty(), which is populated from the chunking stage, not the embedding stage), making the failure completely silent.
fix: Changed Android EmbeddingProvider.embed() stub in AppManager.kt to return List(texts.size * 384) { 0.0f } instead of emptyList(). This matches the Rust NullEmbeddingProvider contract and allows the pipeline to run structurally end-to-end. Real XNNPACK embeddings can replace this stub later; zero-vectors mean all document chunks will have equal cosine distance from any query, so retrieval will return some chunks but not semantically ranked ones — acceptable as placeholder behavior.
verification: Fix traced through both failure paths: (1) EmbeddingComplete: end=(i+1)*384 <= N*384 for all i<N, so all vectors are now added to HNSW index. (2) SendMessage: query_emb.len()=384 > 0, so !query_emb.is_empty() guard passes and retrieval runs.
files_changed: [android/app/src/main/java/com/example/confidentialapp/AppManager.kt]

## Bulk Re-Verification (2026-07-28)

**Verdict:** SUPERSEDED
**Evidence:** Phase 11 MobileEmbeddingProvider.kt:79-93 ONNX Runtime replaces stub
**Verified by:** /gsd-debug bulk re-check vs current HEAD (post-v2.0 + local-LLM work)
