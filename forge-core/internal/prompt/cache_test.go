package prompt

import (
	"os"
	"path/filepath"
	"strings"
	"sync"
	"testing"
)

// writeFile is a tiny test helper: write content to repo-relative path, mkdir-p'ing dirs.
func writeFile(t *testing.T, root, rel, content string) {
	t.Helper()
	p := filepath.Join(root, rel)
	if err := os.MkdirAll(filepath.Dir(p), 0o755); err != nil {
		t.Fatal(err)
	}
	if err := os.WriteFile(p, []byte(content), 0o644); err != nil {
		t.Fatal(err)
	}
}

// adrLane / constraintsLane / taskLane pull a single named lane out of a ctx slice so a
// test can compare ONE lane across calls (e.g. assert the ADR/AGENTS lanes are identical
// while the ROADMAP lane changed). They match on the lane's leading marker — the same
// prefixes Gather/GatherCached emit — and return "" when the lane is absent.
func laneWithPrefix(ctx []string, prefix string) string {
	for _, c := range ctx {
		if strings.HasPrefix(c, prefix) {
			return c
		}
	}
	return ""
}
func adrLane(ctx []string) string {
	return laneWithPrefix(ctx, "Architecture decisions")
}
func constraintsLane(ctx []string) string {
	return laneWithPrefix(ctx, "Engineering constraints")
}
func taskLane(ctx []string) string {
	return laneWithPrefix(ctx, "Current task")
}

// CONCURRENCY GUARD (the gap a fresh reviewer's -race run caught end-to-end): under the
// OPT-IN parallel orchestrator, a fan-out wave's phases each call GatherCached at once. The
// invariant lanes must build EXACTLY ONCE under the lock (no torn fields, no double build) —
// this drives N goroutines through the REAL GatherCached and asserts builds==1. Run with
// `-race`: before the cache mutex this raced on c.built/c.adrDocs/c.constraintsBlock.
func TestGatherCached_ConcurrentBuildIsRaceFreeAndBuildsOnce(t *testing.T) {
	dir := t.TempDir()
	writeFile(t, dir, ".agent/ROADMAP.md", "# Roadmap\n\nbuild it")
	writeFile(t, dir, ".agent/AGENTS.md", "# AGENTS\n- 单文件 ≤ 500 行\n- 依赖方向向内")
	writeFile(t, dir, "docs/adr/0001-stack.md", "# ADR 0001: Go core stack")
	writeFile(t, dir, "docs/adr/0002-layer.md", "# ADR 0002: layering inward")

	cache := NewContextCache()
	var wg sync.WaitGroup
	for i := 0; i < 16; i++ {
		wg.Add(1)
		go func() { defer wg.Done(); _ = GatherCached(cache, dir, "implementer") }()
	}
	wg.Wait()
	if cache.builds != 1 {
		t.Errorf("invariant lanes must build EXACTLY ONCE even under 16 concurrent callers; builds=%d", cache.builds)
	}
}

// ★ THE LOAD-BEARING TEST ★ — the ROADMAP is NEVER cached, so an implementer that ticks a
// [x] into it (rewriting the body) is seen on the very next GatherCached. We simulate that:
// write ROADMAP v1, GatherCached, REWRITE ROADMAP to v2 (the implementer's edit), GatherCached
// again — the second call MUST carry the v2 body. The SAME test pins the other half of the
// contract: across that same pair of calls the ADR and AGENTS (invariant) lanes are byte-for-
// byte identical (they ARE cached). One test, both guarantees: volatile lane fresh, invariant
// lanes memoized.
func TestGatherCached_RoadmapNeverStaleButInvariantLanesCached(t *testing.T) {
	dir := t.TempDir()
	writeFile(t, dir, ".agent/ROADMAP.md", "# Roadmap\n\nVERSION-ONE build the alpha widget")
	writeFile(t, dir, ".agent/AGENTS.md", "# AGENTS\n- 单文件 ≤ 500 行\n- 依赖方向向内\n- 无硬编码 secret")
	writeFile(t, dir, "docs/adr/0001-stack.md", "# ADR 0001: Go core stack")
	writeFile(t, dir, "docs/adr/0002-layer.md", "# ADR 0002: layering inward")

	cache := NewContextCache()
	query := "implementer implementer"

	first := GatherCached(cache, dir, query)
	if !strings.Contains(taskLane(first), "VERSION-ONE build the alpha widget") {
		t.Fatalf("first call must carry ROADMAP v1; got task lane: %q", taskLane(first))
	}

	// The implementer ticks a [x] / advances the board: the ROADMAP body is REWRITTEN.
	writeFile(t, dir, ".agent/ROADMAP.md", "# Roadmap\n\nVERSION-TWO build the beta widget")

	second := GatherCached(cache, dir, query)
	// ★ STALE-KILL ★: the rewritten body MUST appear; the old one MUST be gone.
	if !strings.Contains(taskLane(second), "VERSION-TWO build the beta widget") {
		t.Errorf("ROADMAP must NEVER be cached — second call must read the rewritten v2 body; got task lane: %q", taskLane(second))
	}
	if strings.Contains(taskLane(second), "VERSION-ONE") {
		t.Errorf("second call still shows stale ROADMAP v1 — the task lane was wrongly cached; got: %q", taskLane(second))
	}
	// ★ INVARIANT-CACHED ★: ADR + AGENTS lanes are identical across the two calls.
	if adrLane(first) != adrLane(second) {
		t.Errorf("ADR lane must be cached (identical across calls):\n first=%q\nsecond=%q", adrLane(first), adrLane(second))
	}
	if constraintsLane(first) != constraintsLane(second) {
		t.Errorf("constraints lane must be cached (identical across calls):\n first=%q\nsecond=%q", constraintsLane(first), constraintsLane(second))
	}
	// And the invariant lanes are actually present (not vacuously equal because both empty).
	if adrLane(second) == "" || constraintsLane(second) == "" {
		t.Errorf("expected non-empty ADR and constraints lanes; adr=%q constraints=%q", adrLane(second), constraintsLane(second))
	}
}

// CACHE EQUIVALENCE — GatherCached and Gather must return the EXACT SAME ctx slice for the
// same (root, query). The cache changes WHERE the invariant inputs come from (memo vs disk),
// never WHAT the prompt says. Verified against the real repo (the same corpus Gather's own
// tests use) AND a temp repo, so both a populated and a synthetic layout are pinned.
func TestGatherCached_EqualsGather(t *testing.T) {
	repo := testRepoRoot(t)
	cases := []struct {
		name, root, query string
	}{
		{"real-repo", repo, "stack polyglot go"},
		{"real-repo-other-query", repo, "reviewer reviewer"},
	}
	for _, tc := range cases {
		t.Run(tc.name, func(t *testing.T) {
			cache := NewContextCache()
			want := Gather(tc.root, tc.query)
			got := GatherCached(cache, tc.root, tc.query)
			if !equalSlices(want, got) {
				t.Errorf("GatherCached must equal Gather for (%s, %q):\n Gather=%#v\nCached=%#v", tc.root, tc.query, want, got)
			}
			// And a SECOND cached call (now served from memo) must STILL equal Gather —
			// the memo path produces the same bytes as the first, disk-backed call.
			if got2 := GatherCached(cache, tc.root, tc.query); !equalSlices(want, got2) {
				t.Errorf("second (memoized) GatherCached must still equal Gather:\n Gather=%#v\nCached2=%#v", want, got2)
			}
		})
	}
}

// Temp-repo equivalence: a synthetic layout with ROADMAP + AGENTS + ADRs, asserting the full
// three-lane ctx slice from GatherCached is identical to Gather's, host-agnostically.
func TestGatherCached_EqualsGather_TempRepo(t *testing.T) {
	dir := t.TempDir()
	writeFile(t, dir, ".agent/ROADMAP.md", "# Roadmap\n\nimplement the multiplier")
	writeFile(t, dir, ".agent/AGENTS.md", "# AGENTS\n- rule alpha\n- rule beta")
	writeFile(t, dir, "docs/adr/0001-a.md", "# ADR 0001: alpha decision")
	writeFile(t, dir, "docs/adr/0002-b.md", "# ADR 0002: beta decision")

	for _, q := range []string{"alpha decision", "implementer build", ""} {
		want := Gather(dir, q)
		got := GatherCached(NewContextCache(), dir, q)
		if !equalSlices(want, got) {
			t.Errorf("temp-repo equivalence failed for query %q:\n Gather=%#v\nCached=%#v", q, want, got)
		}
	}
}

// HIT-COUNT — N GatherCached calls build the invariant lanes EXACTLY ONCE. The observable is
// the cache's own builds counter (incremented only when ensureBuilt actually scans the disk).
// After many calls it must read 1: the readdir(docs/adr) + readFile(AGENTS.md) happened a
// single time, every later call served from the memo. (The ROADMAP is re-read each call by
// design and is intentionally NOT what this counts.)
func TestGatherCached_BuildsInvariantLanesExactlyOnce(t *testing.T) {
	dir := t.TempDir()
	writeFile(t, dir, ".agent/ROADMAP.md", "# Roadmap\n\nthe task")
	writeFile(t, dir, ".agent/AGENTS.md", "# AGENTS\n- rule one\n- rule two")
	writeFile(t, dir, "docs/adr/0001-x.md", "# ADR 0001: decision x")

	cache := NewContextCache()
	for i := 0; i < 5; i++ {
		GatherCached(cache, dir, "decision x")
	}
	if cache.builds != 1 {
		t.Errorf("invariant lanes must build exactly once across N calls; builds=%d", cache.builds)
	}

	// Invalidate forces ONE rebuild on the next call (the v2 hook); prove the seam works.
	cache.Invalidate()
	GatherCached(cache, dir, "decision x")
	if cache.builds != 2 {
		t.Errorf("Invalidate must force exactly one rebuild on the next call; builds=%d", cache.builds)
	}
}

// QUERY-SENSITIVE — what is cached is the ADR DOC INPUT, not the retrieval OUTPUT. So two
// calls on the SAME cache with DIFFERENT queries can select different ADRs: the retriever
// re-runs each call over the memoized doc set. With today's tiny top-K≈all corpus the lane
// holds both ADRs, so we assert the RANKED ORDER differs — a query matching one ADR ranks it
// first; the other query ranks the other first. Identical doc set, query-driven output —
// proving the cache memoizes input, not output.
func TestGatherCached_QuerySensitiveOverCachedDocs(t *testing.T) {
	dir := t.TempDir()
	writeFile(t, dir, ".agent/AGENTS.md", "# AGENTS\n- rule one")
	// Two ADR titles with disjoint discriminating terms.
	writeFile(t, dir, "docs/adr/0001-db.md", "# ADR 0001: database persistence postgres")
	writeFile(t, dir, "docs/adr/0002-net.md", "# ADR 0002: networking transport grpc")

	cache := NewContextCache()
	dbCtx := GatherCached(cache, dir, "database postgres")
	netCtx := GatherCached(cache, dir, "networking grpc")

	dbADR := adrLane(dbCtx)
	netADR := adrLane(netCtx)
	// The doc INPUT was built once (same cache), yet the ranked ADR lane differs by query:
	// the "database" query ranks ADR 0001 ahead of 0002; the "networking" query reverses it.
	if idx0001, idx0002 := strings.Index(dbADR, "0001"), strings.Index(dbADR, "0002"); idx0001 < 0 || idx0002 < 0 || idx0001 > idx0002 {
		t.Errorf("database query should rank ADR 0001 (postgres) first; got ADR lane: %q", dbADR)
	}
	if idx0001, idx0002 := strings.Index(netADR, "0001"), strings.Index(netADR, "0002"); idx0001 < 0 || idx0002 < 0 || idx0002 > idx0001 {
		t.Errorf("networking query should rank ADR 0002 (grpc) first; got ADR lane: %q", netADR)
	}
	if dbADR == netADR {
		t.Errorf("different queries over the SAME cached docs must be able to differ (retrieval re-runs); both were: %q", dbADR)
	}
	// And the build happened ONCE despite the two differing-query calls (input cached, not output).
	if cache.builds != 1 {
		t.Errorf("query-varying calls must still build the doc input only once; builds=%d", cache.builds)
	}
}

// equalSlices reports whether two string slices are element-wise equal (length + order +
// content). Used by the equivalence tests to pin GatherCached == Gather byte-for-byte.
func equalSlices(a, b []string) bool {
	if len(a) != len(b) {
		return false
	}
	for i := range a {
		if a[i] != b[i] {
			return false
		}
	}
	return true
}
