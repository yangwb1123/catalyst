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
// the gate-set + reviewer it must distill to. The table is package-scope so the
// test body stays a thin loop (arch-check function-length budget).
type effectiveCase struct {
	name      string
	mode      string
	lifecycle string
	wantGates string // sorted, comma-joined
	wantRev   bool
}

var effectiveCases = []effectiveCase{
	// ── mode baselines under the freest lifecycle (idea) — the mode shows through ──
	// explorer is the headline lean posture: only "does it run", no reviewer.
	{"explorer/idea lean set", "explorer", "idea", "build,lint", false},
	{"balanced/idea", "balanced", "idea", "build,complexity,lint,test", true},
	{"engineering/idea full", "engineering", "idea", allGatesSorted, true},
	// cto produces no code → empty gate-set, but reviewer ON (reviews the docs).
	{"cto/idea no code gates", "cto", "idea", "", true},

	// ── lifecycle tightens the floor (can only ADD gates / force reviewer) ──
	{"explorer/mvp adds build+lint floor", "explorer", "mvp", "build,lint", false},
	{"explorer/growth raises floor", "explorer", "growth", "build,complexity,lint,test", false},

	// ── ★ production override: the safety veto forces FULL gates + reviewer ★ ──
	// explorer is the loosest mode; production must STILL force every gate + reviewer.
	{"explorer/production OVERRIDE full", "explorer", "production", allGatesSorted, true},
	{"balanced/production full", "balanced", "production", allGatesSorted, true},
	{"engineering/production full", "engineering", "production", allGatesSorted, true},
	{"cto/production full", "cto", "production", allGatesSorted, true},

	// ── ★ fail-safe: unknown/empty input over-enforces (full + reviewer) ★ ──
	{"unknown mode → full", "bogus-mode", "mvp", allGatesSorted, true},
	{"empty mode → full", "", "mvp", allGatesSorted, true},
	{"unknown lifecycle → full", "explorer", "bogus-lifecycle", allGatesSorted, true},
	{"empty lifecycle → full", "explorer", "", allGatesSorted, true},
	{"both unknown → full", "bogus", "bogus", allGatesSorted, true},
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
