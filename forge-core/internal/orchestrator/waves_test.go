package orchestrator

import (
	"strings"
	"testing"

	"forgeos/forge-core/internal/asset"
)

// ph builds a phase with a name and optional deps — terse fixture for the planner.
func ph(name string, deps ...string) asset.Phase {
	return asset.Phase{Name: name, Agent: name, DependsOn: deps}
}

// No depends_on anywhere -> a SINGLE wave of every phase (the pure fan-out case the
// parallel engine runs all-at-once).
func TestWaves_NoDeps_SingleFanOutWave(t *testing.T) {
	w, err := Waves([]asset.Phase{ph("a"), ph("b"), ph("c")})
	if err != nil {
		t.Fatalf("Waves: %v", err)
	}
	if len(w) != 1 || len(w[0]) != 3 {
		t.Fatalf("want one wave of 3, got %v", w)
	}
	// Authored order preserved within the wave.
	if w[0][0] != 0 || w[0][1] != 1 || w[0][2] != 2 {
		t.Errorf("authored order not preserved: %v", w[0])
	}
}

// A linear chain a<-b<-c -> three single-phase waves in order.
func TestWaves_LinearChain(t *testing.T) {
	w, err := Waves([]asset.Phase{ph("a"), ph("b", "a"), ph("c", "b")})
	if err != nil {
		t.Fatalf("Waves: %v", err)
	}
	want := [][]int{{0}, {1}, {2}}
	if !equalWaves(w, want) {
		t.Errorf("chain waves = %v, want %v", w, want)
	}
}

// A diamond: a<-b, a<-c, then d<-b,c. Wave 0 = a; wave 1 = b,c (independent, parallel);
// wave 2 = d. This is the shape the parallelism actually helps (b and c concurrent).
func TestWaves_Diamond(t *testing.T) {
	w, err := Waves([]asset.Phase{ph("a"), ph("b", "a"), ph("c", "a"), ph("d", "b", "c")})
	if err != nil {
		t.Fatalf("Waves: %v", err)
	}
	want := [][]int{{0}, {1, 2}, {3}}
	if !equalWaves(w, want) {
		t.Errorf("diamond waves = %v, want %v", w, want)
	}
}

// FAIL-CLOSED: an unknown dependency name aborts with NO waves (never a guessed order).
func TestWaves_UnknownDep_Errors(t *testing.T) {
	_, err := Waves([]asset.Phase{ph("a"), ph("b", "ghost")})
	if err == nil || !strings.Contains(err.Error(), "ghost") {
		t.Errorf("unknown dep must error naming it; got %v", err)
	}
}

// FAIL-CLOSED: a self-dependency is malformed.
func TestWaves_SelfDep_Errors(t *testing.T) {
	_, err := Waves([]asset.Phase{ph("a", "a")})
	if err == nil || !strings.Contains(err.Error(), "itself") {
		t.Errorf("self-dep must error; got %v", err)
	}
}

// FAIL-CLOSED: a cycle a<->b among the phases aborts (a governance runtime must never
// silently run a malformed graph), naming the stranded phases.
func TestWaves_Cycle_Errors(t *testing.T) {
	_, err := Waves([]asset.Phase{ph("a", "b"), ph("b", "a")})
	if err == nil || !strings.Contains(err.Error(), "cycle") {
		t.Fatalf("cycle must error; got %v", err)
	}
	if !strings.Contains(err.Error(), "a") || !strings.Contains(err.Error(), "b") {
		t.Errorf("cycle error should name the stranded phases; got %v", err)
	}
}

func equalWaves(got, want [][]int) bool {
	if len(got) != len(want) {
		return false
	}
	for i := range got {
		if len(got[i]) != len(want[i]) {
			return false
		}
		for j := range got[i] {
			if got[i][j] != want[i][j] {
				return false
			}
		}
	}
	return true
}
