// ForgeOS scorecard writer — the learning-loop synthesis step that closes
// Eval -> scorecard -> Router (see .agent/routing/scorecard.schema.yml).
//
// One acceptance decide() verdict == one binary sample: a task is `accepted`
// iff decide().accepted === true. The acceptance honesty contract already
// excludes N/A (decide() never counts N/A toward satisfaction), so an accepted
// verdict is a clean positive signal and a rejected one a clean negative — no
// re-derivation of N/A handling is needed here.
//
// Beyond the binary, a verdict can now carry a TRAJECTORY: how many rounds the
// task took to converge (`iterations`) and whether a reviewer bounced it back
// (`reworked`). These roll up into the schema's optional `avg_iterations` and
// `rework_rate` so the Router can prefer not just models that pass, but models
// that pass *cheaply* (fewer rounds, less rework). HONESTY: the auto-collection
// of trajectory (iteration count from `forge evolve`'s .forge/trace.jsonl, a
// reviewer's bounce-back) is NOT wired yet — today these fields arrive only when
// the orchestrator/CLI injects them on the verdict. A verdict that omits them is
// the legacy binary shape and degrades gracefully (see synthesize/merge below).
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

// Floor an iteration count at 1.0 (schema `avg_iterations.min: 1.0` — a green
// task took at least one round). Non-finite collapses to the 1.0 floor rather
// than poisoning the average; the caller decides whether a value exists at all.
function floorIterations(n) {
  if (!Number.isFinite(n)) return 1;
  return n < 1 ? 1 : n;
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

// Roll the trajectory side-channel of a batch into two accumulators:
//   - reworked: how many verdicts were bounced back by a reviewer
//       (`reworked === true`); a verdict that omits the field is NOT reworked,
//       so legacy `{accepted}`-only batches yield rework_rate 0.
//   - iterSum / iterCount: sum and count of `iterations` over ACCEPTED tasks
//       that actually report a finite count. avg_iterations is a "rounds to
//       green" metric, so only accepted tasks contribute; a count is floored at
//       1. iterCount === 0 means "no trajectory data" -> the field is omitted.
function rollTrajectory(verdicts) {
  let reworked = 0;
  let iterSum = 0;
  let iterCount = 0;
  for (const v of verdicts) {
    if (!v) continue;
    if (v.reworked === true) reworked += 1;
    if (v.accepted === true && Number.isFinite(v.iterations)) {
      iterSum += floorIterations(v.iterations);
      iterCount += 1;
    }
  }
  return { reworked, iterSum, iterCount };
}

// synthesize({model, task_type, verdicts, updated_at}) -> one scorecard entry.
//
// - one verdict == one task == one binary sample (accepted true/false)
// - quality_score = #accepted / #tasks   (samples === 0 -> 0, no divide-by-zero)
// - pass_rate mirrors quality_score (first-pass acceptance rate over this batch)
// - rework_rate = #reworked / #tasks     (samples === 0 -> 0); a verdict missing
//   `reworked` counts as not reworked, so a legacy binary batch yields 0
// - avg_iterations = mean `iterations` over accepted tasks that report it; when
//   NO accepted task reports a count the field is OMITTED (honest "no data",
//   never a fabricated default) so the Router can tell "1.0 rounds" from "unknown"
// - updated_at is the caller-supplied ISO-8601 timestamp (NOT Date.now() here)
//
// BACKWARD COMPAT: for verdicts shaped `{accepted}` only, quality_score /
// pass_rate / samples are bit-for-bit unchanged, rework_rate is 0, and
// avg_iterations is absent — the new fields never perturb the old contract.
//
// Output satisfies scorecard.schema.yml: {model, task_type, quality_score in
// [0,1], samples >= 0, updated_at}, plus the optional pass_rate / rework_rate /
// avg_iterations enrichment.
export function synthesize({ model, task_type, verdicts = [], updated_at }) {
  const samples = verdicts.length;
  const accepted = countAccepted(verdicts);
  const quality_score = samples === 0 ? 0 : clamp01(accepted / samples);
  const { reworked, iterSum, iterCount } = rollTrajectory(verdicts);
  const rework_rate = samples === 0 ? 0 : clamp01(reworked / samples);
  const entry = {
    model,
    task_type,
    quality_score,
    samples,
    pass_rate: quality_score,
    rework_rate,
    updated_at,
  };
  // Only emit avg_iterations when at least one accepted task reported a count;
  // floor at 1.0 to honor the schema minimum.
  if (iterCount > 0) entry.avg_iterations = floorIterations(iterSum / iterCount);
  return entry;
}

// Reconstruct how many ACCEPTED samples backed a row's avg_iterations. avg is a
// mean over green tasks, so the honest weight is the accepted-sample count =
// quality_score * samples (floored at 0). A row that never recorded an average
// (no avg_iterations) contributes nothing to the merged average.
function acceptedWeight(rowSamples, qualityScore) {
  const n = Math.max(0, rowSamples || 0);
  return n * clamp01(qualityScore);
}

// merge(existing, incoming) -> sample-weighted mean of quality_score with the
// sample counts summed. This is how a stored (model × task_type) row absorbs a
// fresh batch without re-reading history: weight each score by its sample count.
//
//   q = (q_e * n_e + q_i * n_i) / (n_e + n_i)   (both n === 0 -> q = 0)
//
// rework_rate folds the same way (a rate over all samples, weighted by total
// samples). avg_iterations folds weighted by ACCEPTED samples instead (it is a
// mean over green tasks only) and survives only if at least one side recorded
// it; otherwise it is omitted, matching synthesize's "no data" contract.
//
// Identity/metadata (model, task_type) and updated_at come from `incoming` —
// it is the newer observation. Counts are floored at 0 so a malformed negative
// `samples` can never drag the merged total below zero. Missing fields on a
// legacy stored row (no rework_rate / avg_iterations) are treated as absent and
// handled gracefully.
export function merge(existing, incoming) {
  const ne = Math.max(0, existing.samples || 0);
  const ni = Math.max(0, incoming.samples || 0);
  const total = ne + ni;
  const qe = clamp01(existing.quality_score);
  const qi = clamp01(incoming.quality_score);
  const quality_score = total === 0 ? 0 : clamp01((qe * ne + qi * ni) / total);

  // rework_rate: total-sample-weighted; a legacy row missing it reads as 0.
  const re = clamp01(existing.rework_rate ?? 0);
  const ri = clamp01(incoming.rework_rate ?? 0);
  const rework_rate = total === 0 ? 0 : clamp01((re * ne + ri * ni) / total);

  const merged = {
    model: incoming.model ?? existing.model,
    task_type: incoming.task_type ?? existing.task_type,
    quality_score,
    samples: total,
    pass_rate: quality_score,
    rework_rate,
    updated_at: incoming.updated_at ?? existing.updated_at,
  };

  // avg_iterations: accepted-sample-weighted, only over rows that recorded it.
  const we = Number.isFinite(existing.avg_iterations) ? acceptedWeight(ne, existing.quality_score) : 0;
  const wi = Number.isFinite(incoming.avg_iterations) ? acceptedWeight(ni, incoming.quality_score) : 0;
  const wTotal = we + wi;
  if (wTotal > 0) {
    const ae = floorIterations(existing.avg_iterations);
    const ai = floorIterations(incoming.avg_iterations);
    merged.avg_iterations = floorIterations((ae * we + ai * wi) / wTotal);
  }
  return merged;
}
