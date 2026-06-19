package mode

import (
	"sort"
	"strings"
	"testing"
)

// sortedGates renders a Policy's gate-set as a stable, comparable string so
// tests assert on CONTENT regardless of slice order.
func sortedGates(p Policy) string {
	g := make([]string, len(p.Gates))
	copy(g, p.Gates)
	sort.Strings(g)
	return strings.Join(g, ",")
}

const allGatesSorted = "arch,build,complexity,lint,security,test"

// effectiveCase is one Effective expectation: the (mode, lifecycle) input and
// the gate-set + reviewer + evolve-depth it must distill to. The table is
// package-scope so the test body stays a thin loop (arch-check function-length
// budget).
type effectiveCase struct {
	name       string
	mode       string
	lifecycle  string
	wantGates  string // sorted, comma-joined
	wantRev    bool
	wantEvolve string // EvolveDepth label
}

var effectiveCases = []effectiveCase{
	// ── mode baselines under the freest lifecycle (idea) — the mode shows through ──
	// explorer is the headline lean posture: only "does it run", no reviewer,
	// shallowest (opportunistic) evolve loop.
	{"explorer/idea lean set", "explorer", "idea", "build,lint", false, EvolveOpportunistic},
	{"balanced/idea", "balanced", "idea", "build,complexity,lint,test", true, EvolveStandard},
	{"engineering/idea full", "engineering", "idea", allGatesSorted, true, EvolveThorough},
	// cto produces no code → empty gate-set, reviewer ON (reviews docs), advisory evolve.
	{"cto/idea no code gates", "cto", "idea", "", true, EvolveAdvisory},

	// ── lifecycle tightens the floor (can only ADD gates / force reviewer) ──
	// idea/mvp/growth impose NO evolve floor, so the mode's depth passes through.
	{"explorer/mvp adds build+lint floor", "explorer", "mvp", "build,lint", false, EvolveOpportunistic},
	{"explorer/growth raises floor", "explorer", "growth", "build,complexity,lint,test", false, EvolveOpportunistic},

	// ── ★ production override: safety veto forces FULL gates + reviewer + ≥standard evolve ★ ──
	// explorer is the loosest mode; production STILL forces every gate + reviewer
	// AND raises its opportunistic loop to standard (no prototype-shallow loop in prod).
	{"explorer/production OVERRIDE full", "explorer", "production", allGatesSorted, true, EvolveStandard},
	{"balanced/production full", "balanced", "production", allGatesSorted, true, EvolveStandard},
	// engineering is already thorough (≥ standard): the floor RAISES, never caps down.
	{"engineering/production stays thorough", "engineering", "production", allGatesSorted, true, EvolveThorough},
	// cto's advisory is raised to the standard floor under production.
	{"cto/production full", "cto", "production", allGatesSorted, true, EvolveStandard},

	// ── ★ fail-safe: unknown/empty input over-enforces (full + reviewer + standard evolve) ★ ──
	{"unknown mode → full", "bogus-mode", "mvp", allGatesSorted, true, EvolveStandard},
	{"empty mode → full", "", "mvp", allGatesSorted, true, EvolveStandard},
	{"unknown lifecycle → full", "explorer", "bogus-lifecycle", allGatesSorted, true, EvolveStandard},
	{"empty lifecycle → full", "explorer", "", allGatesSorted, true, EvolveStandard},
	{"both unknown → full", "bogus", "bogus", allGatesSorted, true, EvolveStandard},
}

func TestEffective(t *testing.T) {
	for _, c := range effectiveCases {
		t.Run(c.name, func(t *testing.T) {
			got := Effective(c.mode, c.lifecycle)
			if g := sortedGates(got); g != c.wantGates {
				t.Errorf("Effective(%q,%q) gates = %q, want %q", c.mode, c.lifecycle, g, c.wantGates)
			}
			if got.Reviewer != c.wantRev {
				t.Errorf("Effective(%q,%q) reviewer = %v, want %v", c.mode, c.lifecycle, got.Reviewer, c.wantRev)
			}
			if got.EvolveDepth != c.wantEvolve {
				t.Errorf("Effective(%q,%q) evolve-depth = %q, want %q", c.mode, c.lifecycle, got.EvolveDepth, c.wantEvolve)
			}
		})
	}
}

// The production override is the load-bearing safety claim: explorer+production
// must permit EVERY gate, not the lean explorer subset. Asserted explicitly (in
// addition to the table) because it is the whole point of the lifecycle veto.
func TestEffective_ProductionOverridesLooseMode(t *testing.T) {
	p := Effective("explorer", "production")
	for _, g := range fullGates {
		if !p.Allows(g) {
			t.Errorf("explorer+production must allow gate %q (production forces full enforcement)", g)
		}
	}
	if !p.Reviewer {
		t.Error("explorer+production must force the reviewer on")
	}
	// And it must be STRICTLY more than bare explorer (the override actually fired).
	if lean := Effective("explorer", "idea"); lean.Allows(GateSecurity) {
		t.Error("sanity: bare explorer should NOT allow security; the production case proves the override added it")
	}
}

// evolveCase pairs an EvolveDepth label with the default --max-iter it must map
// to (the only concrete v1 behavior evolve-depth drives).
type evolveCase struct {
	depth   string
	wantMax int
}

// The depth→max-iter contract, asserted directly on Policy.EvolveMaxIter so the
// mapping is pinned independent of how Effective derives the depth: advisory→1,
// opportunistic→2, standard→5, thorough→10, and — load-bearing for back-compat —
// an unknown/empty depth → 5 (the historical default).
var evolveMaxCases = []evolveCase{
	{EvolveAdvisory, 1},
	{EvolveOpportunistic, 2},
	{EvolveStandard, 5},
	{EvolveThorough, 10},
	{"", 5},            // zero-value Policy → the conservative legacy default
	{"bogus-depth", 5}, // unrecognized label → same conservative fallback
}

func TestPolicy_EvolveMaxIter(t *testing.T) {
	for _, c := range evolveMaxCases {
		if got := (Policy{EvolveDepth: c.depth}).EvolveMaxIter(); got != c.wantMax {
			t.Errorf("Policy{EvolveDepth:%q}.EvolveMaxIter() = %d, want %d", c.depth, got, c.wantMax)
		}
	}
	// The zero-value Policy must report the legacy default explicitly (back-compat:
	// a no-mode-gating Policy yields the same --max-iter the CLI used before mode).
	if got := (Policy{}).EvolveMaxIter(); got != 5 {
		t.Errorf("zero-value Policy.EvolveMaxIter() = %d, want 5 (legacy default)", got)
	}
}

// End-to-end through Effective: the per-mode default iteration budget the CLI
// will adopt. explorer→opportunistic→2 (shallow), engineering→thorough→10 (deep),
// cto→advisory→1, balanced→standard→5 — the headline mode-drives-evolve claim.
func TestEffective_EvolveMaxIterByMode(t *testing.T) {
	cases := map[string]int{"explorer": 2, "balanced": 5, "engineering": 10, "cto": 1}
	for m, want := range cases {
		if got := Effective(m, "idea").EvolveMaxIter(); got != want {
			t.Errorf("Effective(%q,\"idea\").EvolveMaxIter() = %d, want %d", m, got, want)
		}
	}
}

// ★ production tightens evolve depth the same way it tightens gates: explorer's
// shallow opportunistic (2) is RAISED to standard (5) in production, but
// engineering's already-deeper thorough (10) is NOT capped down to it. This is
// the evolve-dimension half of the production safety veto.
func TestEffective_ProductionRaisesEvolveFloor(t *testing.T) {
	if got := Effective("explorer", "production").EvolveMaxIter(); got != 5 {
		t.Errorf("explorer+production evolve max-iter = %d, want 5 (raised to standard floor)", got)
	}
	// Sanity: bare explorer is shallower, proving production actually raised it.
	if got := Effective("explorer", "idea").EvolveMaxIter(); got != 2 {
		t.Errorf("bare explorer evolve max-iter = %d, want 2 (the floor must have lifted prod to 5)", got)
	}
	// thorough is already ≥ standard: the floor raises, never lowers.
	if got := Effective("engineering", "production").EvolveMaxIter(); got != 10 {
		t.Errorf("engineering+production evolve max-iter = %d, want 10 (floor must not cap thorough)", got)
	}
}

// Allows reflects exactly the gate-set: explorer permits lint/build and rejects
// the heavier gates, so the orchestrator's intersection drops complexity/arch/
// security for explorer.
func TestPolicy_Allows(t *testing.T) {
	p := Effective("explorer", "idea")
	for _, g := range []string{GateLint, GateBuild} {
		if !p.Allows(g) {
			t.Errorf("explorer should allow %q", g)
		}
	}
	for _, g := range []string{GateTest, GateComplexity, GateArch, GateSecurity} {
		if p.Allows(g) {
			t.Errorf("explorer should NOT allow %q (lean set)", g)
		}
	}
}

// The returned Policy must own its slice: mutating it must not corrupt the
// shared baseline table for the next caller.
func TestEffective_ReturnedSliceIsOwned(t *testing.T) {
	p := Effective("explorer", "idea")
	if len(p.Gates) > 0 {
		p.Gates[0] = "MUTATED"
	}
	again := Effective("explorer", "idea")
	if again.Allows("MUTATED") || !again.Allows(GateLint) {
		t.Errorf("baseline table was corrupted by a caller mutation; got %v", again.Gates)
	}
}

// The zero-value Policy must be distinguishable from every real mode so the
// orchestrator can treat it as "no gating configured" (full back-compat). A real
// mode either carries gates (explorer/balanced/engineering) or, for the only
// empty-gate mode (cto), forces the reviewer on — so no real (mode,lifecycle)
// collapses to the zero value {nil, false}.
func TestEffective_NeverCollapsesToZeroValue(t *testing.T) {
	modes := []string{"explorer", "balanced", "engineering", "cto"}
	lifecycles := []string{"idea", "mvp", "growth", "production"}
	for _, m := range modes {
		for _, l := range lifecycles {
			p := Effective(m, l)
			if len(p.Gates) == 0 && !p.Reviewer {
				t.Errorf("Effective(%q,%q) collapsed to the zero value {nil,false}; "+
					"orchestrator could not tell real-policy from no-gating", m, l)
			}
		}
	}
}
