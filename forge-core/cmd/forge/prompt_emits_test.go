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

// labelOnlyEmitsAgents are agents whose emits: declarations are a conceptual
// LABEL, not a literal Write() path, so the write-scope check below must not
// assert a path prefix for them — and must say so EXPLICITLY rather than
// silently falling through, which is exactly the blind spot an independent
// review of this test caught (an agent absent from readonlyAgentWriteScope
// produced zero assertions, indistinguishable from "correctly scoped"):
//   - qa: its own card (.agent/agents/qa.md) states "不写代码文件;仅产 QA
//     报告" — it never writes ANY file; evolve.yml's `evaluate` phase emits:
//     eval-scorecard.md is shorthand for "this phase's judgment feeds the Eval
//     scorecard" (harness/scorecard-update.mjs, a separate machine-authored
//     JSON artifact), not a file qa itself produces.
//   - planner: readonlyAgentWriteScope's own doc comment already covers this
//     one (a fixed FILE, not a directory) — build.yml's `emits: task-plan.md`
//     is a label read back by emitsContext as a bare repo-root filename, not
//     planner's real write target (.agent/CURRENT_SPRINT.md).
var labelOnlyEmitsAgents = map[string]bool{
	"qa": true,
}

// End-to-end regression: every phase across the four real spine/loop workflows
// that is BOTH readonly (so its write scope is enforced by readonlyToolScope)
// AND declares emits: must EITHER declare paths that actually fall within its
// agent's documented write scope (readonlyAgentWriteScope), OR be one of the
// explicitly documented label-only exceptions above. Otherwise the emits-
// content injection this file wires (priorEmitsOf/emitsFilesFor/
// buildPromptWithEmits) silently finds nothing in a real run: emitsContext
// resolves each emits path relative to repoRoot, so a bare filename like
// "security-review.md" never matches a file the agent actually wrote at
// docs/review/security-review.md — this was a real, confirmed bug (review.yml/
// discover.yml/design.yml/evolve.yml all declared bare filenames until fixed
// alongside this wiring). An agent with NEITHER a readonlyAgentWriteScope
// entry NOR a labelOnlyEmitsAgents exemption is a hard failure, not a silent
// skip — that combination means either a real unscoped-write bug or an
// undocumented label case that needs to be added to one of the two maps above.
func TestEndToEnd_WorkflowEmitsMatchAgentWriteScope(t *testing.T) {
	if _, err := exec.LookPath("python3"); err != nil {
		t.Skip("python3 not available")
	}
	root := repoRoot()
	if root == "" {
		t.Skip("not running inside the ForgeOS repo (no harness/yaml2json.py)")
	}
	for _, name := range []string{"discover", "design", "review", "evolve"} {
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
			if len(patterns) == 0 {
				if !labelOnlyEmitsAgents[p.Agent] {
					t.Errorf("%s.yml phase %q: agent %q declares emits %v but has NO readonlyAgentWriteScope entry and is not in labelOnlyEmitsAgents — either add its write-scope pattern, or if its emits are a conceptual label (like qa/planner), add it to labelOnlyEmitsAgents with a documented reason", name, p.Name, p.Agent, p.Emits)
				}
				continue
			}
			for _, pat := range patterns {
				if !strings.HasSuffix(pat, "/**") {
					continue // fixed-file agent (e.g. planner) — emits is a label, not a path
				}
				prefix := strings.TrimPrefix(strings.TrimSuffix(pat, "**"), "/")
				for _, emit := range p.Emits {
					if !strings.HasPrefix(emit, prefix) {
						t.Errorf("%s.yml phase %q (agent %s): emits %q must be under %q (its enforced write scope), or emitsContext will never find it in a real run", name, p.Name, p.Agent, emit, prefix)
					}
				}
			}
		}
	}
}
