package main

import (
	"os"
	"os/exec"
	"path/filepath"
	"strings"
	"testing"

	"forgeos/forge-core/internal/asset"
	"forgeos/forge-core/internal/converge"
)

// buildPrompt must embed the role, phase, routed tier, and the agent's card so
// a real `claude -p` invocation has the full instruction. reviewer floors to
// opus regardless of mode.
func TestBuildPrompt_EmbedsRolePhaseTier(t *testing.T) {
	p := asset.Phase{Name: "reviewer", Agent: "reviewer"}
	got := buildPrompt("/home/u1/catalyst", p, "balanced")
	for _, want := range []string{`"reviewer" agent`, "phase=reviewer", "tier=opus"} {
		if !strings.Contains(got, want) {
			t.Errorf("prompt missing %q", want)
		}
	}
}

// A missing card must not break prompt assembly — it degrades to a marker.
func TestBuildPrompt_MissingCardDegrades(t *testing.T) {
	p := asset.Phase{Name: "ghost", Agent: "no-such-agent"}
	got := buildPrompt("/home/u1/catalyst", p, "balanced")
	if !strings.Contains(got, "no role card found") {
		t.Errorf("expected missing-card marker; got: %.80s", got)
	}
}

func TestRun_NoArgsIsUsageError(t *testing.T) {
	if code := run(nil); code != 2 {
		t.Errorf("run(nil) = %d, want 2", code)
	}
}

// repoRoot finds the ForgeOS repo root (the dir holding harness/yaml2json.py),
// or "" when the test is not running inside the repo.
func repoRoot() string {
	dir, err := os.Getwd()
	if err != nil {
		return ""
	}
	for {
		if _, err := os.Stat(filepath.Join(dir, "harness", "yaml2json.py")); err == nil {
			return dir
		}
		parent := filepath.Dir(dir)
		if parent == dir {
			return ""
		}
		dir = parent
	}
}

// End to end: load the REAL build.yml via the yaml2json shim + asset loader and
// assert the typed criteria evaluate per-criterion as expected. build.yml's
// all_of items are objects ({metric, operator, threshold/value}), so this proves
// the typed UnmarshalJSON + converge dispatch works on the production asset.
// Skips when python3 is unavailable or not inside the repo.
func TestEndToEnd_BuildYmlCriteria(t *testing.T) {
	if _, err := exec.LookPath("python3"); err != nil {
		t.Skip("python3 not available")
	}
	root := repoRoot()
	if root == "" {
		t.Skip("not running inside the ForgeOS repo (no harness/yaml2json.py)")
	}
	wf, err := loadWorkflow(root, "build")
	if err != nil {
		t.Fatalf("load build.yml: %v", err)
	}
	if wf.Stop.Type != "conjunction" {
		t.Fatalf("build.yml stop type = %q, want conjunction", wf.Stop.Type)
	}
	if len(wf.Stop.AllOf) != 2 {
		t.Fatalf("build.yml all_of = %d criteria, want 2 (objects)", len(wf.Stop.AllOf))
	}
	// They must be parsed as typed objects, not bare strings.
	if wf.Stop.AllOf[0].Metric != "roadmap_completion" || wf.Stop.AllOf[0].Raw != "" {
		t.Errorf("criterion[0] = %+v, want typed roadmap_completion object", wf.Stop.AllOf[0])
	}
	if wf.Stop.AllOf[1].Metric != "gates_status" || wf.Stop.AllOf[1].Value != "green" {
		t.Errorf("criterion[1] = %+v, want gates_status==green", wf.Stop.AllOf[1])
	}

	// Fully met: 100% roadmap + green gates => all criteria met.
	met, allMet := converge.Evaluate(wf.Stop.AllOf, converge.Signals{RoadmapCompletion: 1.0, GatesGreen: true})
	if !allMet || !met[0].Met || !met[1].Met {
		t.Errorf("100%%+green should meet every criterion; got %+v", met)
	}
	// Partial roadmap, green gates => roadmap unmet, gate met, not converged.
	mixed, conv := converge.Evaluate(wf.Stop.AllOf, converge.Signals{RoadmapCompletion: 0.5, GatesGreen: true})
	if conv || mixed[0].Met || !mixed[1].Met {
		t.Errorf("50%%+green: roadmap unmet & gate met & not converged; got %+v", mixed)
	}
	// 100% roadmap, red gates => roadmap met, gate unmet, not converged.
	red, conv2 := converge.Evaluate(wf.Stop.AllOf, converge.Signals{RoadmapCompletion: 1.0, GatesGreen: false})
	if conv2 || !red[0].Met || red[1].Met {
		t.Errorf("100%%+red: roadmap met & gate unmet & not converged; got %+v", red)
	}
}
