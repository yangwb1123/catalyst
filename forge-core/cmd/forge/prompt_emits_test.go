package main

import (
	"os"
	"os/exec"
	"path/filepath"
	"strings"
	"testing"

	"forgeos/forge-core/internal/asset"
	"forgeos/forge-core/internal/orchestrator"
)

// priorEmitsOf is the data-driven (phase -> union of earlier phases' declared
// emits) lookup wired into buildPromptWithEmits: it returns every EARLIER
// phase's emits (by index, not by graph position), never the named phase's own
// or a later phase's, and nil for a workflow with no emits: anywhere.
func TestPriorEmitsOf_ReturnsOnlyEarlierPhasesEmits(t *testing.T) {
	wf, err := asset.LoadWorkflowJSON([]byte(`{
	  "stage": "review",
	  "phases": [
	    {"name": "security-review", "agent": "security-engineer", "required_gates": [], "emits": ["security-review.md"]},
	    {"name": "distributed-review", "agent": "distributed-engineer", "required_gates": [], "emits": ["distributed-review.md"]},
	    {"name": "cto-synthesis", "agent": "cto", "required_gates": []}
	  ],
	  "stop_condition": {"type": "external", "all_of": []}
	}`))
	if err != nil {
		t.Fatalf("load fixture: %v", err)
	}
	lookup := priorEmitsOf(wf)
	if got := lookup("security-review"); got != nil {
		t.Errorf("the first phase has no earlier phase; got %v", got)
	}
	if got := lookup("distributed-review"); len(got) != 1 || got[0] != "security-review.md" {
		t.Errorf("distributed-review must see only security-review's emits; got %v", got)
	}
	if got := lookup("cto-synthesis"); len(got) != 2 || got[0] != "security-review.md" || got[1] != "distributed-review.md" {
		t.Errorf("cto-synthesis must see both earlier phases' emits, in order; got %v", got)
	}
}

// emitsFilesFor is the nil-safe wrapper agentExecutor's Build closure calls
// unconditionally; a nil priorEmits lookup (every existing test call site
// before this wiring) must yield nil, not a panic.
func TestEmitsFilesFor_NilLookupIsSafe(t *testing.T) {
	if got := emitsFilesFor(nil, "any-phase"); got != nil {
		t.Errorf("a nil priorEmits lookup must yield nil, not panic; got %v", got)
	}
	lookup := func(name string) []string { return []string{"x.md"} }
	if got := emitsFilesFor(lookup, "any-phase"); len(got) != 1 || got[0] != "x.md" {
		t.Errorf("a real lookup must be called through unchanged; got %v", got)
	}
}

// End-to-end: a workflow where an earlier phase declares emits: [...] and the
// declared file exists on disk must have its content actually appear in a
// LATER phase's spawned prompt, driven through the full production chain
// (priorEmitsOf -> agentExecutor -> Build -> buildPromptWithEmits ->
// appendArtifactContext) with zero manual emits wiring at the call site —
// this is the fix for the "review.yml emits-content injection is dead code"
// finding: before this wiring, agentExecutor's Build closure always called
// buildPrompt (which passes emits=nil), so a downstream reviewer never saw an
// earlier reviewer's actual findings, only would have to be told to go read
// the file itself.
func TestAgentExecutor_PriorEmitsWiredIntoRealPrompt(t *testing.T) {
	root := t.TempDir()
	artifactPath := filepath.Join(root, "security-review.md")
	if err := os.WriteFile(artifactPath, []byte("SECRET-SECURITY-FINDING-31415"), 0o644); err != nil {
		t.Fatalf("seed emits artifact: %v", err)
	}
	wf, err := asset.LoadWorkflowJSON([]byte(`{
	  "stage": "review",
	  "phases": [
	    {"name": "security-review", "agent": "security-engineer", "required_gates": [], "emits": ["security-review.md"]},
	    {"name": "distributed-review", "agent": "distributed-engineer", "required_gates": []}
	  ],
	  "stop_condition": {"type": "external", "all_of": []}
	}`))
	if err != nil {
		t.Fatalf("load fixture: %v", err)
	}

	o := runOpts{root: root, executor: "command", agentCmd: "claude"}
	ex := agentExecutor(o, func(string) {}, nil, unbudgetedTier("balanced"), nil, nil, nil, nil, nil, nil, nil, nil, priorEmitsOf(wf))
	ce, ok := ex.(orchestrator.CommandExecutor)
	if !ok {
		t.Fatalf("--executor=command must select orchestrator.CommandExecutor, got %T", ex)
	}
	argv := ce.Build(asset.Phase{Name: "distributed-review", Agent: "distributed-engineer"}, "balanced")
	promptArg := argv[len(argv)-1]
	if !strings.Contains(promptArg, "SECRET-SECURITY-FINDING-31415") {
		t.Errorf("distributed-review's spawned prompt must contain security-review's emitted content; got tail %.800s", promptArg)
	}

	// The FIRST phase in the workflow (no earlier phase) must NOT see its own
	// not-yet-produced emits — proving priorEmits is strictly "earlier", not
	// "everything declared in the workflow".
	selfArgv := ce.Build(asset.Phase{Name: "security-review", Agent: "security-engineer", Emits: []string{"security-review.md"}}, "balanced")
	selfPromptArg := selfArgv[len(selfArgv)-1]
	if strings.Contains(selfPromptArg, "SECRET-SECURITY-FINDING-31415") {
		t.Errorf("security-review must not see its own not-yet-produced emits; got tail %.800s", selfPromptArg)
	}
}

func emitMatchesScope(emit string, patterns []string) bool {
	normalized := "/" + strings.TrimPrefix(filepath.ToSlash(filepath.Clean(emit)), "/")
	for _, pattern := range patterns {
		if strings.HasSuffix(pattern, "/**") {
			if strings.HasPrefix(normalized, strings.TrimSuffix(pattern, "**")) {
				return true
			}
		} else if normalized == pattern {
			return true
		}
	}
	return false
}

// Every readonly phase's declared artifacts must be literal paths covered by
// the same write scope enforced in the spawned agent CLI.
func TestEndToEnd_WorkflowEmitsMatchAgentWriteScope(t *testing.T) {
	if _, err := exec.LookPath("python3"); err != nil {
		t.Skip("python3 not available")
	}
	root := repoRoot()
	if root == "" {
		t.Skip("not running inside the ForgeOS repo (no harness/yaml2json.py)")
	}
	for _, name := range []string{"discover", "design", "review", "build", "deploy", "rollback", "evolve"} {
		wf, err := loadWorkflow(root, name)
		if err != nil {
			t.Fatalf("load %s.yml: %v", name, err)
		}
		phases := wf.Phases
		if wf.Loop != nil {
			phases = wf.Loop.Phases
		}
		if len(phases) == 0 {
			t.Fatalf("%s.yml: no phases loaded", name)
		}
		for _, p := range phases {
			if !p.Readonly || len(p.Emits) == 0 {
				continue
			}
			patterns := readonlyAgentWriteScope[p.Agent]
			if p.Agent == "release-engineer" {
				patterns, err = releaseEmitPermissionPatterns(root, p)
				if err != nil {
					t.Errorf("%s.yml phase %q: exact release emit scope: %v", name, p.Name, err)
					continue
				}
			}
			if len(patterns) == 0 {
				t.Errorf("%s.yml phase %q: agent %q declares emits %v but has no readonly write scope", name, p.Name, p.Agent, p.Emits)
				continue
			}
			for _, emit := range p.Emits {
				if !emitMatchesScope(emit, patterns) {
					t.Errorf("%s.yml phase %q (agent %s): emit %q is outside enforced scopes %v", name, p.Name, p.Agent, emit, patterns)
				}
			}
		}
	}
}
