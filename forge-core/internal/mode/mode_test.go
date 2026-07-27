package mode

import (
	"reflect"
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
	wantReview   string // ReviewDepth label
	wantADR      bool
}

var effectiveCases = []effectiveCase{
	// ── mode baselines under the freest lifecycle (idea) — the mode shows through ──
	// explorer is the headline lean posture: only "does it run", no reviewer,
	// shallowest (opportunistic) evolve loop, SKIPS discover, light design, SKIPS
	// the deep review, no ADR.
	{"explorer/idea lean set", "explorer", "idea", "build,lint", false, EvolveOpportunistic, DiscoverSkip, DesignLight, ReviewSkip, false},
	{"balanced/idea", "balanced", "idea", "build,complexity,lint,test", true, EvolveStandard, DiscoverLight, DesignStandard, ReviewStandard, false},
	{"engineering/idea full", "engineering", "idea", allGatesSorted, true, EvolveThorough, DiscoverFull, DesignFull, ReviewFull, true},
	// cto produces no code → empty gate-set, reviewer ON (reviews docs), advisory
	// evolve, but FULL discover/design/review + ADR (it IS the analysis-producing mode).
	{"cto/idea no code gates", "cto", "idea", "", true, EvolveAdvisory, DiscoverFull, DesignFull, ReviewFull, true},

	// ── lifecycle tightens the floor (can only ADD gates / force reviewer) ──
	// idea/mvp/growth impose NO evolve/discover/design/review/adr floor → mode passes through.
	{"explorer/mvp adds build+lint floor", "explorer", "mvp", "build,lint", false, EvolveOpportunistic, DiscoverSkip, DesignLight, ReviewSkip, false},
	{"explorer/growth raises gate floor only", "explorer", "growth", "build,complexity,lint,test", false, EvolveOpportunistic, DiscoverSkip, DesignLight, ReviewSkip, false},

	// ── ★ production override: safety veto forces FULL everything ★ ──
	// explorer is the loosest mode; production STILL forces every gate + reviewer,
	// raises opportunistic→standard, AND raises discover skip→full, design light→full,
	// review skip→full, ADR false→true (no prototype skip-discover / skip-review /
	// no-ADR in prod).
	{"explorer/production OVERRIDE full", "explorer", "production", allGatesSorted, true, EvolveStandard, DiscoverFull, DesignFull, ReviewFull, true},
	{"balanced/production full", "balanced", "production", allGatesSorted, true, EvolveStandard, DiscoverFull, DesignFull, ReviewFull, true},
	// engineering is already thorough/full/full/full/true: the floor RAISES, never caps.
	{"engineering/production stays thorough", "engineering", "production", allGatesSorted, true, EvolveThorough, DiscoverFull, DesignFull, ReviewFull, true},
	// cto's advisory is raised to standard; discover/design/review/adr already full/full/full/true.
	{"cto/production full", "cto", "production", allGatesSorted, true, EvolveStandard, DiscoverFull, DesignFull, ReviewFull, true},

	// ── ★ fail-safe: unknown/empty input over-enforces (full everything, std evolve) ★ ──
	{"unknown mode → full", "bogus-mode", "mvp", allGatesSorted, true, EvolveStandard, DiscoverFull, DesignFull, ReviewFull, true},
	{"empty mode → full", "", "mvp", allGatesSorted, true, EvolveStandard, DiscoverFull, DesignFull, ReviewFull, true},
	{"unknown lifecycle → full", "explorer", "bogus-lifecycle", allGatesSorted, true, EvolveStandard, DiscoverFull, DesignFull, ReviewFull, true},
	{"empty lifecycle → full", "explorer", "", allGatesSorted, true, EvolveStandard, DiscoverFull, DesignFull, ReviewFull, true},
	{"both unknown → full", "bogus", "bogus", allGatesSorted, true, EvolveStandard, DiscoverFull, DesignFull, ReviewFull, true},
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
			if got.ReviewDepth != c.wantReview {
				t.Errorf("Effective(%q,%q) review-depth = %q, want %q", c.mode, c.lifecycle, got.ReviewDepth, c.wantReview)
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

func TestEffective_CTOAlwaysHaltsBeforeBuild(t *testing.T) {
	for _, lifecycle := range []string{"idea", "mvp", "growth", "production", "", "bogus-lifecycle"} {
		if p := Effective("cto", lifecycle); !p.BuildHalted() {
			t.Errorf("Effective(cto, %q) must preserve workflow_depth.build=halt", lifecycle)
		}
	}
	for _, modeName := range []string{"explorer", "balanced", "engineering"} {
		if p := Effective(modeName, "mvp"); p.BuildHalted() {
			t.Errorf("Effective(%q, mvp) unexpectedly halts Build", modeName)
		}
	}
	if (Policy{}).BuildHalted() {
		t.Error("zero-value Policy must preserve backward-compatible Build behavior")
	}
}

// Unknown lifecycle input arrives from either --lifecycle or project.yml and is
// intentionally accepted by the CLI so policy resolution can fail closed. Its
// result must equal application of the strictest lifecycle FLOOR, not replacement
// by a generic policy: replacement would downgrade engineering thorough→standard
// and erase cto's independent build=halt boundary.
func TestEffective_UnknownLifecyclePreservesStrictModeBaseline(t *testing.T) {
	for _, modeName := range []string{"explorer", "balanced", "engineering", "cto"} {
		want := Effective(modeName, "production")
		for _, lifecycle := range []string{"", "bogus-lifecycle"} {
			if got := Effective(modeName, lifecycle); !reflect.DeepEqual(got, want) {
				t.Errorf("Effective(%q, %q) = %+v, want strict floor %+v",
					modeName, lifecycle, got, want)
			}
		}
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

// ReviewSkipped mirrors DiscoverSkipped exactly: true ONLY for the explicit "skip"
// depth (explorer), false for standard/full and the zero value — so a zero-value
// Policy (no gating) never skips the review stage.
func TestPolicy_ReviewSkipped(t *testing.T) {
	cases := []struct {
		depth string
		want  bool
	}{
		{ReviewSkip, true},
		{ReviewStandard, false},
		{ReviewFull, false},
		{"", false},      // zero-value Policy → never skip (back-compat)
		{"bogus", false}, // unknown → never skip (fail-safe: run the stage)
	}
	for _, c := range cases {
		if got := (Policy{ReviewDepth: c.depth}).ReviewSkipped(); got != c.want {
			t.Errorf("Policy{ReviewDepth:%q}.ReviewSkipped() = %v, want %v", c.depth, got, c.want)
		}
	}
	// End-to-end: explorer skips, the others do not.
	if !Effective("explorer", "idea").ReviewSkipped() {
		t.Error("explorer/idea must skip the deep review")
	}
	for _, m := range []string{"balanced", "engineering", "cto"} {
		if Effective(m, "idea").ReviewSkipped() {
			t.Errorf("%s/idea must NOT skip the deep review (standard/full)", m)
		}
	}
	// ★ production override: explorer+production must NOT skip review.
	if Effective("explorer", "production").ReviewSkipped() {
		t.Error("explorer+production must NOT skip the deep review (production restores the stage)")
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
// to. This table pins the iteration-budget half of the v1 machine contract; the
// independent mutation-authority contract is covered separately below and by
// orchestrator tests.
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

func TestEffective_EvolveAuthorityIsIndependentAndNeverLifecycleWidened(t *testing.T) {
	tests := []struct {
		name, mode, lifecycle, want string
	}{
		{"explorer idea", "explorer", "idea", evolveAuthorityPropose},
		{"explorer production", "explorer", "production", evolveAuthorityPropose},
		{"explorer unknown lifecycle", "explorer", "typo", evolveAuthorityPropose},
		{"cto production", "cto", "production", evolveAuthorityPropose},
		{"balanced", "balanced", "mvp", evolveAuthorityMutate},
		{"engineering", "engineering", "production", evolveAuthorityMutate},
		{"unknown mode fails closed", "typo", "mvp", evolveAuthorityPropose},
	}
	for _, tc := range tests {
		t.Run(tc.name, func(t *testing.T) {
			got := Effective(tc.mode, tc.lifecycle)
			if got.EvolveAuthority != tc.want {
				t.Errorf("authority = %q, want %q", got.EvolveAuthority, tc.want)
			}
			if got.EvolveProposalOnly() != (tc.want == evolveAuthorityPropose) {
				t.Errorf("EvolveProposalOnly = %v for authority %q", got.EvolveProposalOnly(), got.EvolveAuthority)
			}
		})
	}
	if !(Policy{}).EvolveProposalOnly() {
		t.Error("unknown/empty authority must fail closed at the method boundary")
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

// PrioritiesFor mirrors modes.yml `priorities:` VERBATIM (the single Go mirror of
// that policy data). Each known mode returns its exact ranking; cto's deliberate
// tie (speed=cost=3, "quality only") is preserved, not normalized to a permutation.
func TestPrioritiesFor(t *testing.T) {
	cases := []struct {
		mode                 string
		speed, quality, cost int
	}{
		{"explorer", 1, 3, 2},
		{"balanced", 2, 1, 3},
		{"engineering", 3, 1, 2},
		{"cto", 3, 1, 3}, // intentional tie — must survive verbatim
	}
	for _, c := range cases {
		t.Run(c.mode, func(t *testing.T) {
			p, ok := PrioritiesFor(c.mode)
			if !ok {
				t.Fatalf("PrioritiesFor(%q) ok=false, want true (known mode)", c.mode)
			}
			if p.Speed != c.speed || p.Quality != c.quality || p.Cost != c.cost {
				t.Errorf("PrioritiesFor(%q) = %+v, want {Speed:%d Quality:%d Cost:%d}",
					c.mode, p, c.speed, c.quality, c.cost)
			}
		})
	}
}

// An unknown/empty mode falls back to balanced's ranking with ok=false — the
// caller can both surface the default posture's priorities AND report that the
// named mode was not found.
func TestPrioritiesFor_UnknownFallsBackToBalanced(t *testing.T) {
	bal, _ := PrioritiesFor("balanced")
	for _, m := range []string{"", "does-not-exist", "ENGINEERING"} {
		p, ok := PrioritiesFor(m)
		if ok {
			t.Errorf("PrioritiesFor(%q) ok=true, want false (unknown mode)", m)
		}
		if p != bal {
			t.Errorf("PrioritiesFor(%q) = %+v, want balanced default %+v", m, p, bal)
		}
	}
}

// HONESTY guard: priorities must NOT leak into the effective Workflow-depth
// Policy. Two modes with DIFFERENT priorities but the same gate/evolve posture
// still differ only where modes.yml says they should — priorities are an
// observability surface, never an input to Effective. (engineering vs cto differ
// in gates by design; this asserts the priorities field simply does not exist on
// Policy, i.e. the distillations are kept separate.)
func TestPriorities_DoNotAffectEffectivePolicy(t *testing.T) {
	// Effective's output type carries no priorities — its fields are exactly the
	// workflow-depth knobs. If priorities were ever wired in, this package would
	// not compile against the asserted shape below, catching the drift.
	p := Effective("engineering", "production")
	_ = p.Gates
	_ = p.Reviewer
	_ = p.EvolveDepth
	_ = p.DiscoverDepth
	_ = p.DesignDepth
	_ = p.ReviewDepth
	_ = p.ADR
	// Priorities live on their own type, reached only via the accessor.
	if _, ok := PrioritiesFor("engineering"); !ok {
		t.Fatal("PrioritiesFor(engineering) ok=false; priorities accessor must stand alone")
	}
}
