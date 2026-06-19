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

// effectiveCase is one Effective expectation: the (mode, lifecycle) input and the
// full distilled policy — gate-set + reviewer + evolve-depth + discover/design
// depth + adr. The table is package-scope so the test body stays a thin loop
// (arch-check function-length budget).
type effectiveCase struct {
	name         string
	mode         string
	lifecycle    string
	wantGates    string // sorted, comma-joined
	wantRev      bool
	wantEvolve   string // EvolveDepth label
	wantDiscover string // DiscoverDepth label
	wantDesign   string // DesignDepth label
	wantADR      bool
}

var effectiveCases = []effectiveCase{
	// ── mode baselines under the freest lifecycle (idea) — the mode shows through ──
	// explorer is the headline lean posture: only "does it run", no reviewer,
	// shallowest (opportunistic) evolve loop, SKIPS discover, light design, no ADR.
	{"explorer/idea lean set", "explorer", "idea", "build,lint", false, EvolveOpportunistic, DiscoverSkip, DesignLight, false},
	{"balanced/idea", "balanced", "idea", "build,complexity,lint,test", true, EvolveStandard, DiscoverLight, DesignStandard, false},
	{"engineering/idea full", "engineering", "idea", allGatesSorted, true, EvolveThorough, DiscoverFull, DesignFull, true},
	// cto produces no code → empty gate-set, reviewer ON (reviews docs), advisory
	// evolve, but FULL discover/design + ADR (it IS the analysis-producing mode).
	{"cto/idea no code gates", "cto", "idea", "", true, EvolveAdvisory, DiscoverFull, DesignFull, true},

	// ── lifecycle tightens the floor (can only ADD gates / force reviewer) ──
	// idea/mvp/growth impose NO evolve/discover/design/adr floor → mode passes through.
	{"explorer/mvp adds build+lint floor", "explorer", "mvp", "build,lint", false, EvolveOpportunistic, DiscoverSkip, DesignLight, false},
	{"explorer/growth raises gate floor only", "explorer", "growth", "build,complexity,lint,test", false, EvolveOpportunistic, DiscoverSkip, DesignLight, false},

	// ── ★ production override: safety veto forces FULL everything ★ ──
	// explorer is the loosest mode; production STILL forces every gate + reviewer,
	// raises opportunistic→standard, AND raises discover skip→full, design light→full,
	// ADR false→true (no prototype skip-discover / no-ADR in prod).
	{"explorer/production OVERRIDE full", "explorer", "production", allGatesSorted, true, EvolveStandard, DiscoverFull, DesignFull, true},
	{"balanced/production full", "balanced", "production", allGatesSorted, true, EvolveStandard, DiscoverFull, DesignFull, true},
	// engineering is already thorough/full/full/true: the floor RAISES, never caps.
	{"engineering/production stays thorough", "engineering", "production", allGatesSorted, true, EvolveThorough, DiscoverFull, DesignFull, true},
	// cto's advisory is raised to standard; discover/design/adr already full/full/true.
	{"cto/production full", "cto", "production", allGatesSorted, true, EvolveStandard, DiscoverFull, DesignFull, true},

	// ── ★ fail-safe: unknown/empty input over-enforces (full everything, std evolve) ★ ──
	{"unknown mode → full", "bogus-mode", "mvp", allGatesSorted, true, EvolveStandard, DiscoverFull, DesignFull, true},
	{"empty mode → full", "", "mvp", allGatesSorted, true, EvolveStandard, DiscoverFull, DesignFull, true},
	{"unknown lifecycle → full", "explorer", "bogus-lifecycle", allGatesSorted, true, EvolveStandard, DiscoverFull, DesignFull, true},
	{"empty lifecycle → full", "explorer", "", allGatesSorted, true, EvolveStandard, DiscoverFull, DesignFull, true},
	{"both unknown → full", "bogus", "bogus", allGatesSorted, true, EvolveStandard, DiscoverFull, DesignFull, true},
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
			if got.DiscoverDepth != c.wantDiscover {
				t.Errorf("Effective(%q,%q) discover-depth = %q, want %q", c.mode, c.lifecycle, got.DiscoverDepth, c.wantDiscover)
			}
			if got.DesignDepth != c.wantDesign {
				t.Errorf("Effective(%q,%q) design-depth = %q, want %q", c.mode, c.lifecycle, got.DesignDepth, c.wantDesign)
			}
			if got.ADR != c.wantADR {
				t.Errorf("Effective(%q,%q) adr = %v, want %v", c.mode, c.lifecycle, got.ADR, c.wantADR)
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

// DiscoverSkipped is the orchestrator's switch for eliding the discover stage: it
// is true ONLY for the explicit "skip" depth (explorer), and false for light/full
// and the zero value — so a zero-value Policy (no gating) never skips the stage.
func TestPolicy_DiscoverSkipped(t *testing.T) {
	cases := []struct {
		depth string
		want  bool
	}{
		{DiscoverSkip, true},
		{DiscoverLight, false},
		{DiscoverFull, false},
		{"", false},      // zero-value Policy → never skip (back-compat)
		{"bogus", false}, // unknown → never skip (fail-safe: run the stage)
	}
	for _, c := range cases {
		if got := (Policy{DiscoverDepth: c.depth}).DiscoverSkipped(); got != c.want {
			t.Errorf("Policy{DiscoverDepth:%q}.DiscoverSkipped() = %v, want %v", c.depth, got, c.want)
		}
	}
	// End-to-end: explorer skips, the others do not.
	if !Effective("explorer", "idea").DiscoverSkipped() {
		t.Error("explorer/idea must skip discovery")
	}
	for _, m := range []string{"balanced", "engineering", "cto"} {
		if Effective(m, "idea").DiscoverSkipped() {
			t.Errorf("%s/idea must NOT skip discovery (light/full)", m)
		}
	}
}

// ★ The production override extends to discover/design/adr the same way it does
// gates: explorer's skip-discover / light-design / no-ADR is RAISED to full / full
// / true under production — a loose prototype posture cannot apply in prod. And
// engineering's already-full baseline is not capped down (the floor only raises).
func TestEffective_ProductionRaisesDiscoverDesignADR(t *testing.T) {
	p := Effective("explorer", "production")
	if p.DiscoverSkipped() {
		t.Error("explorer+production must NOT skip discovery (production restores the stage)")
	}
	if p.DiscoverDepth != DiscoverFull || p.DesignDepth != DesignFull || !p.ADR {
		t.Errorf("explorer+production = discover %q/design %q/adr %v, want full/full/true",
			p.DiscoverDepth, p.DesignDepth, p.ADR)
	}
	// Sanity: bare explorer is the loose posture, proving production actually raised it.
	if lean := Effective("explorer", "idea"); !lean.DiscoverSkipped() || lean.ADR {
		t.Errorf("bare explorer should skip discovery and write no ADR; got discover=%q adr=%v",
			lean.DiscoverDepth, lean.ADR)
	}
	// engineering already full/full/true: the floor raises, never lowers.
	if e := Effective("engineering", "production"); e.DiscoverDepth != DiscoverFull || !e.ADR {
		t.Errorf("engineering+production must stay full discovery + ADR; got discover=%q adr=%v", e.DiscoverDepth, e.ADR)
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
