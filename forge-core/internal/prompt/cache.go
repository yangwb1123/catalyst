// cache.go — a RUN-SCOPED memo of the INVARIANT prompt context lanes, so a run that
// builds the prompt once per phase does not re-read the unchanging inputs (the ADR
// title set + the AGENTS.md hard constraints) on every single phase.
//
// ★ HONESTY — what v1 buys, and what it does NOT ★
//   - It saves LOCAL I/O only: a readdir of docs/adr + a firstHeading read per ADR,
//     and one readFile of .agent/AGENTS.md — microseconds, amortized across a run's
//     phases. That is the WHOLE v1 win, and it is marginal.
//   - It does NOT save a single claude token. The prompt TEXT is byte-for-byte the
//     same as the uncached path (GatherCached returns the identical ctx slice as
//     Gather), so the full prompt is still fed to the API every phase. Token cost is
//     unchanged. Anyone reading this must not mistake a local memo for an API saving.
//   - The REAL token saving is v2 work: wiring the claude API's prompt-caching
//     (cache_control marking the STABLE prefix = role card + ADRs + AGENTS so the
//     vendor reuses it across calls instead of re-billing it). This local cache is
//     the data-shape DRESS REHEARSAL for that step: it is exactly the split between
//     the "stable prefix" (ADRs + AGENTS, memoized here) and the "volatile tail"
//     (the ROADMAP task + the query-driven ADR retrieval, recomputed every call). v2
//     reuses that same boundary at the API layer; v1 proves the boundary is sound.
//
// ★★ CORRECTNESS INVARIANT — any agent-writable file NEVER enters this cache ★★
// The ROADMAP is the live task board: a print-mode agent ticks `[x]` into it
// (commit e6d2e13) and the NEXT phase must read the new body. So the ROADMAP is
// NEVER memoized — and it is not merely "not cached", it is structurally impossible
// to cache here: ContextCache HOLDS NO ROADMAP FIELD. With no field there is no slot
// to hold a stale value, so no code path can ever serve an old ROADMAP. This is the
// type system enforcing the honesty rule, not a comment asking nicely.
//
// (v2 will let an asset opt into caching its ADRs via a writes_adr flag; the day an
// ADR becomes agent-writable, the same invariant demands the ADR cache be dropped on
// write — Invalidate() is the hook for exactly that. v1 has no writer of ADRs, so v1
// never calls it; it exists so the v2 author finds the seam already cut.)

package prompt

import (
	"strings"
	"sync"
)

// ContextCache memoizes ONLY the invariant context lanes for the duration of ONE run
// (it is created per-run by the caller and never shared across runs — see the cmd/forge
// buildRunEngine owner). Its two fields are built lazily on first GatherCached call and
// reused thereafter:
//
//   - adrDocs: the docs/adr title set as []Doc (the input to Retrieve). Built once;
//     the ADR FILES do not change within a run, so the title scan is done a single time.
//   - constraintsBlock: the leading AGENTS.md hard-constraint bullets. Built once; the
//     constitution does not change within a run.
//
// ★ There is DELIBERATELY no ROADMAP field. ★ The task lane is agent-writable and must
// be re-read every phase; encoding that as "no field exists" makes a stale ROADMAP
// unrepresentable (see the file header). built tracks whether the lazy build has run,
// so a repo that legitimately has zero ADRs and no AGENTS.md still builds exactly once
// (and caches the empty result) rather than re-scanning every phase.
//
// CONCURRENCY: mu guards the lazy build + the field reads, so GatherCached is safe under
// the OPT-IN parallel orchestrator (concurrent agent phases each call it once at prompt-
// build time — the "should phases ever parallelize, this would need a guard" the prior
// comment foresaw). The build runs ONCE under the lock; the slow per-phase retrieval runs
// OUTSIDE it over a returned snapshot, so phases still overlap. The serial path takes the
// uncontended lock and is byte-for-byte unchanged.
type ContextCache struct {
	mu               sync.Mutex
	built            bool   // has the lazy build of the invariant lanes run yet?
	adrDocs          []Doc  // memoized ADR title set (Retrieve input); nil is a valid built value.
	constraintsBlock string // memoized AGENTS.md hard constraints; "" is a valid built value.

	// builds counts how many times the invariant lanes were ACTUALLY built from the
	// filesystem (incremented only when ensureBuilt finds built==false and does the scan).
	// It is the in-struct observable the hit-count test reads: N GatherCached calls must
	// leave builds==1, proving the readdir(docs/adr)+readFile(AGENTS.md) ran exactly once
	// across the run. Production never reads it; it is a pure instrumentation field that
	// costs one int and keeps the hit-counter OUT of any package global (an injected,
	// scope-local observable, not a mutable singleton).
	builds int
}

// NewContextCache returns an empty, unbuilt cache. The caller (cmd/forge buildRunEngine)
// creates ONE per run/evolve loop and threads it through every phase's buildPrompt; it is
// never a package-global singleton (that would let a memoized value escape its run's scope
// and bleed an old ADR/AGENTS snapshot into a later, unrelated run).
func NewContextCache() *ContextCache { return &ContextCache{} }

// Invalidate clears the memoized invariant lanes, forcing the next GatherCached to rebuild
// them. v1 NEVER calls this: its only agent-writable context (the ROADMAP) is not cached in
// the first place, so nothing memoized here ever goes stale within a run. It exists for v2:
// once an asset's writes_adr lets an agent edit an ADR mid-run, the writer must call
// Invalidate so the freshly-written ADR is re-scanned instead of served from this stale
// memo. Nil-safe so a caller without a cache cannot panic.
func (c *ContextCache) Invalidate() {
	if c == nil {
		return
	}
	c.mu.Lock()
	defer c.mu.Unlock()
	c.built = false
	c.adrDocs = nil
	c.constraintsBlock = ""
}

// GatherCached is the cache-aware twin of Gather: it returns the SAME context slice (same
// lanes, same order, same formatting — byte-for-byte) but reads the INVARIANT inputs from
// the cache instead of the filesystem after the first call. The lane breakdown:
//
//   - currentTask (ROADMAP): RE-READ EVERY CALL, never cached. This is the load-bearing
//     correctness guarantee — an agent that just ticked a ROADMAP [x] sees the new body on
//     the very next phase. (ContextCache has no field that could hold an old value.)
//   - adrDocs + constraintsBlock: built ONCE (lazy, on first call) into the cache, reused on
//     every later call. These are the invariant lanes the run does not need to re-read.
//   - Retrieve(adrDocs, query, adrTopK): RUN EVERY CALL. What is cached is the DOC INPUT to
//     retrieval (the title set), not the retrieval OUTPUT: query changes per phase, so the
//     selected ADRs may differ call to call even though the underlying docs are the memoized
//     set. Caching the output would be wrong (it varies with query); caching the input is the
//     real, query-independent saving.
//
// A nil cache is NOT accepted here (the caller guarantees a non-nil cache when it opts into
// caching; the nil-cache fallback to plain Gather lives one layer up in buildPrompt). lane
// order is identical to Gather: task, then ADRs, then constraints.
func GatherCached(cache *ContextCache, repoRoot, query string) []string {
	// Snapshot the invariant lanes UNDER the lock (built once), then do the slow per-phase
	// work (task re-read + ADR retrieval) OUTSIDE it so concurrent phases overlap.
	adrDocs, constraintsBlock := cache.invariants(repoRoot)
	var ctx []string
	// (1) TASK lane — ROADMAP, re-read every call (NEVER cached; agent-writable).
	if task := currentTask(repoRoot); task != "" {
		ctx = append(ctx, "Current task — implement what .agent/ROADMAP.md describes:\n"+task)
	}
	// (2) ADR lane — retrieval runs every call over the CACHED doc set (input cached,
	// output recomputed: query varies per phase, so the selected few can differ).
	if adrs := retrieveADRBullets(adrDocs, query); len(adrs) > 0 {
		ctx = append(ctx, "Architecture decisions (ADRs) to respect:\n"+strings.Join(adrs, "\n"))
	}
	// (3) CONSTRAINTS lane — AGENTS.md hard rules, served from the cache (invariant).
	if constraintsBlock != "" {
		ctx = append(ctx, "Engineering constraints (hard, non-negotiable):\n"+constraintsBlock)
	}
	return ctx
}

// invariants lazily populates the INVARIANT lanes exactly once per cache and returns a
// SNAPSHOT of them, all UNDER the lock — so concurrent callers (the parallel orchestrator)
// neither double-build nor race on the fields. adrDocs is never mutated after the build, so
// sharing the returned slice for read-only retrieval across goroutines is safe. After it
// runs, built is true even when a lane is empty (no ADRs / no AGENTS.md), so a bare repo is
// scanned a single time. The ROADMAP is pointedly absent — never an invariant lane.
func (c *ContextCache) invariants(repoRoot string) ([]Doc, string) {
	c.mu.Lock()
	defer c.mu.Unlock()
	if !c.built {
		c.adrDocs = adrDocs(repoRoot)
		c.constraintsBlock = constraints(repoRoot)
		c.built = true
		c.builds++ // observable: counts real filesystem builds (see field doc); ==1 after a run.
	}
	return c.adrDocs, c.constraintsBlock
}

// adrDocs returns the docs/adr title set as []Doc — the CACHEABLE input to retrieval.
// It is the split point of relevantADRs (prompt.go): that function reads adrTitles and
// wraps each as a Doc inline; this isolates the (invariant, cache-once) Doc construction
// so the cache can memoize it, while relevantADRs itself stays byte-for-byte unchanged on
// the non-cached path. Each title becomes one Doc{ID,Text}=title, exactly as relevantADRs
// builds it, so a retrieval over this set is identical to the uncached one.
func adrDocs(repoRoot string) []Doc {
	titles := adrTitles(repoRoot)
	if len(titles) == 0 {
		return nil
	}
	docs := make([]Doc, len(titles))
	for i, t := range titles {
		docs[i] = Doc{ID: t, Text: t}
	}
	return docs
}

// retrieveADRBullets runs the per-call retrieval over an ALREADY-built Doc set and returns
// the selected titles as "- "-prefixed bullets, in the retriever's ranked order — the exact
// tail of relevantADRs (prompt.go), minus the Doc construction (which the cache did once via
// adrDocs). Output formatting matches relevantADRs byte-for-byte, so GatherCached's ADR lane
// equals Gather's. Empty docs yield nil (no ADR lane), same as relevantADRs on no titles.
func retrieveADRBullets(docs []Doc, query string) []string {
	if len(docs) == 0 {
		return nil
	}
	var out []string
	for _, d := range Retrieve(docs, query, adrTopK) {
		out = append(out, "- "+d.Text)
	}
	return out
}
