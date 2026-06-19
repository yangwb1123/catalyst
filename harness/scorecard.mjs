// ForgeOS scorecard writer — the learning-loop synthesis step that closes
// Eval -> scorecard -> Router (see .agent/routing/scorecard.schema.yml).
//
// One acceptance decide() verdict == one binary sample: a task is `accepted`
// iff decide().accepted === true. The acceptance honesty contract already
// excludes N/A (decide() never counts N/A toward satisfaction), so an accepted
// verdict is a clean positive signal and a rejected one a clean negative — no
// re-derivation of N/A handling is needed here.
//
// Design: PURE functions, ZERO I/O. `synthesize()` rolls a batch of verdicts
// into one scorecard entry; `merge()` folds a fresh entry into a stored one as a
// sample-weighted mean. Timestamps are passed IN (never Date.now() inside) so
// the synthesis stays deterministic and unit-testable.

// Clamp a number into [0,1]; non-finite input collapses to 0 (fail-closed: a
// bad score never masquerades as a high one).
function clamp01(n) {
  if (!Number.isFinite(n)) return 0;
  if (n < 0) return 0;
  if (n > 1) return 1;
  return n;
}

// Count the accepted verdicts in a batch. Each verdict is the object returned
// by acceptance.decide(): its `.accepted` boolean is the single binary sample.
function countAccepted(verdicts) {
  let accepted = 0;
  for (const v of verdicts) {
    if (v && v.accepted === true) accepted += 1;
  }
  return accepted;
}

// synthesize({model, task_type, verdicts, updated_at}) -> one scorecard entry.
//
// - one verdict == one task == one binary sample (accepted true/false)
// - quality_score = #accepted / #tasks   (samples === 0 -> 0, no divide-by-zero)
// - pass_rate mirrors quality_score (first-pass acceptance rate over this batch)
// - updated_at is the caller-supplied ISO-8601 timestamp (NOT Date.now() here)
//
// Output satisfies scorecard.schema.yml: {model, task_type, quality_score in
// [0,1], samples >= 0, updated_at}, plus the optional pass_rate enrichment.
export function synthesize({ model, task_type, verdicts = [], updated_at }) {
  const samples = verdicts.length;
  const accepted = countAccepted(verdicts);
  const quality_score = samples === 0 ? 0 : clamp01(accepted / samples);
  return {
    model,
    task_type,
    quality_score,
    samples,
    pass_rate: quality_score,
    updated_at,
  };
}

// merge(existing, incoming) -> sample-weighted mean of quality_score with the
// sample counts summed. This is how a stored (model × task_type) row absorbs a
// fresh batch without re-reading history: weight each score by its sample count.
//
//   q = (q_e * n_e + q_i * n_i) / (n_e + n_i)   (both n === 0 -> q = 0)
//
// Identity/metadata (model, task_type) and updated_at come from `incoming` —
// it is the newer observation. Counts are floored at 0 so a malformed negative
// `samples` can never drag the merged total below zero.
export function merge(existing, incoming) {
  const ne = Math.max(0, existing.samples || 0);
  const ni = Math.max(0, incoming.samples || 0);
  const total = ne + ni;
  const qe = clamp01(existing.quality_score);
  const qi = clamp01(incoming.quality_score);
  const quality_score = total === 0 ? 0 : clamp01((qe * ne + qi * ni) / total);
  return {
    model: incoming.model ?? existing.model,
    task_type: incoming.task_type ?? existing.task_type,
    quality_score,
    samples: total,
    pass_rate: quality_score,
    updated_at: incoming.updated_at ?? existing.updated_at,
  };
}
