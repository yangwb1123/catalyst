package migrate

import (
	"sort"
	"strings"
	"testing"
)

// allGatesSorted is the full gate_catalog from modes.yml, sorted so the gate-set
// assertion is order-independent (same convention as internal/mode's test).
const allGatesSorted = "arch,build,complexity,lint,security,test"

func sortedGates(g []string) string {
	c := make([]string, len(g))
	copy(c, g)
	sort.Strings(c)
	return strings.Join(c, ",")
}

// The Plan must distill modes.yml migrations.explorer_to_engineering.effects
// EXACTLY: from/to modes, the full tightened gate-set, coverage 80, enforce
// block, router floor sonnet, and the three re-enabled workflow dimensions.
func TestExplorerToEngineering_Effects(t *testing.T) {
	p := ExplorerToEngineering()

	if p.From != ModeExplorer || p.To != ModeEngineering {
		t.Errorf("transition = %q->%q, want explorer->engineering", p.From, p.To)
	}
	if g := sortedGates(p.TightenGates); g != allGatesSorted {
		t.Errorf("tighten gates = %q, want the full catalog %q", g, allGatesSorted)
	}
	if p.CoverageThreshold != 80 {
		t.Errorf("coverage threshold = %d, want 80", p.CoverageThreshold)
	}
	if p.Enforce != "block" {
		t.Errorf("enforce = %q, want block (warn->block)", p.Enforce)
	}
	if p.RouterFloor != "sonnet" {
		t.Errorf("router floor = %q, want sonnet (haiku->sonnet)", p.RouterFloor)
	}
	// enable_workflow: discover skip->full, adr true, reviewer true.
	if !p.DiscoverFull || !p.ADR || !p.Reviewer {
		t.Errorf("enable_workflow = {discover_full:%v adr:%v reviewer:%v}, want all true",
			p.DiscoverFull, p.ADR, p.Reviewer)
	}
}

// wantTask is one expected derive_tasks entry from modes.yml.
type wantTask struct {
	id, gate, priority string
}

// The five derived "backfill" tasks must align with modes.yml derive_tasks
// 1:1 — same ids, same gates (add-ci / add-monitoring honestly un-scoped: gate
// == ""), same priorities, same ORDER (ROADMAP injection appends in this order).
func TestExplorerToEngineering_DeriveTasks(t *testing.T) {
	want := []wantTask{
		{"backfill-tests", "test", "high"},
		{"add-ci", "", "high"},
		{"add-monitoring", "", "medium"},
		{"refactor-oversized", "complexity", "medium"},
		{"security-pass", "security", "high"},
	}
	got := ExplorerToEngineering().Tasks
	if len(got) != len(want) {
		t.Fatalf("derived %d tasks, want %d (the 5 modes.yml derive_tasks)", len(got), len(want))
	}
	for i, w := range want {
		g := got[i]
		if g.ID != w.id || g.Gate != w.gate || g.Priority != w.priority {
			t.Errorf("task[%d] = {id:%q gate:%q prio:%q}, want {id:%q gate:%q prio:%q}",
				i, g.ID, g.Gate, g.Priority, w.id, w.gate, w.priority)
		}
		if strings.TrimSpace(g.Title) == "" {
			t.Errorf("task[%d] (%s) has an empty title", i, g.ID)
		}
	}
}

// The returned Plan must OWN its slices: mutating Plan.TightenGates / Plan.Tasks
// must not corrupt the package tables for the next caller (same ownership
// contract internal/mode guarantees).
func TestExplorerToEngineering_ReturnedSlicesOwned(t *testing.T) {
	p := ExplorerToEngineering()
	if len(p.TightenGates) > 0 {
		p.TightenGates[0] = "MUTATED"
	}
	if len(p.Tasks) > 0 {
		p.Tasks[0].ID = "MUTATED"
	}
	again := ExplorerToEngineering()
	if again.TightenGates[0] == "MUTATED" || again.Tasks[0].ID == "MUTATED" {
		t.Errorf("package tables were corrupted by a caller mutation; gates=%v task0=%s",
			again.TightenGates, again.Tasks[0].ID)
	}
}

func TestPromoteToProduction_TransitionMatrix(t *testing.T) {
	modes := []string{ModeExplorer, ModeBalanced, ModeEngineering, ModeCTO}
	lifecycles := []string{
		LifecycleIdea, LifecycleMVP, LifecycleGrowth, LifecycleProduction,
	}
	for _, mode := range modes {
		for _, lifecycle := range lifecycles {
			name := mode + "_" + lifecycle
			t.Run(name, func(t *testing.T) {
				got, err := PromoteToProduction(mode, lifecycle)
				if err != nil {
					t.Fatal(err)
				}
				if got.FromMode != mode || got.FromLifecycle != lifecycle ||
					got.ToLifecycle != LifecycleProduction {
					t.Fatalf("promotion = %+v", got)
				}
				if lifecycle == LifecycleProduction {
					if !got.AlreadyProduction || got.AutoMigration || got.ToMode != mode {
						t.Fatalf("production source was not an exact no-op: %+v", got)
					}
					return
				}
				wantAuto := mode == ModeExplorer
				wantMode := mode
				if wantAuto {
					wantMode = ModeEngineering
				}
				if got.AlreadyProduction || got.AutoMigration != wantAuto ||
					got.ToMode != wantMode {
					t.Fatalf("non-production promotion = %+v, want mode=%s auto=%v",
						got, wantMode, wantAuto)
				}
				if got.AutoMigration && len(got.Migration.Tasks) != 5 {
					t.Fatalf("auto migration tasks = %d, want 5", len(got.Migration.Tasks))
				}
				if !got.AutoMigration && len(got.Migration.Tasks) != 0 {
					t.Fatalf("non-explorer promotion derived tasks: %+v", got.Migration.Tasks)
				}
			})
		}
	}
}

func TestPromoteToProduction_UnknownSelectorsFailClosed(t *testing.T) {
	for _, input := range []struct {
		mode      string
		lifecycle string
	}{
		{"", LifecycleMVP},
		{"unknown", LifecycleMVP},
		{ModeExplorer, ""},
		{ModeExplorer, "unknown"},
	} {
		if _, err := PromoteToProduction(input.mode, input.lifecycle); err == nil {
			t.Errorf("PromoteToProduction(%q, %q) accepted an unknown selector",
				input.mode, input.lifecycle)
		}
	}
}
