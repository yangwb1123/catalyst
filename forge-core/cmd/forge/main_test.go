package main

import (
	"bytes"
	"encoding/json"
	"os"
	"os/exec"
	"path/filepath"
	"strings"
	"testing"
	"time"

	"forgeos/forge-core/internal/asset"
	"forgeos/forge-core/internal/converge"
	"forgeos/forge-core/internal/memory"
	"forgeos/forge-core/internal/persist"
	"forgeos/forge-core/internal/trace"
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

// Backward-compat: the Context Engine upgrade must not drop the hard-constraint
// injection. A prompt built from the REAL repo must still carry the leading
// AGENTS.md constraints (the 500-line cap), exactly as before retrieval+memory.
func TestBuildPrompt_StillInjectsHardConstraints(t *testing.T) {
	got := buildPrompt("/home/u1/catalyst", asset.Phase{Name: "reviewer", Agent: "reviewer"}, "balanced")
	if !strings.Contains(got, "Engineering constraints") || !strings.Contains(got, "500") {
		t.Errorf("hard constraints must still inject after the Context Engine upgrade; got: %.400s", got)
	}
}

// memoryContext: a prompt built in a repo with a seeded memory store must surface
// the recorded gaps/decisions/lessons, so a real agent sees what prior iterations
// learned instead of rediscovering it.
func TestBuildPrompt_IncludesMemoryEntries(t *testing.T) {
	root := t.TempDir()
	seed := []memory.Entry{
		{Kind: memory.KindGap, Topic: "build", Detail: "missing retry on flaky gate", Iteration: 1, CreatedAtUnix: 1},
		{Kind: memory.KindDecision, Topic: "build", Detail: "chose JSONL for the memory store", Iteration: 2, CreatedAtUnix: 2},
	}
	for _, e := range seed {
		if err := memory.Append(memoryPath(root), e); err != nil {
			t.Fatalf("seed memory: %v", err)
		}
	}
	got := buildPrompt(root, asset.Phase{Name: "build", Agent: "implementer"}, "balanced")
	if !strings.Contains(got, "Project memory") {
		t.Errorf("prompt must carry a Project memory block; got: %.400s", got)
	}
	for _, want := range []string{"missing retry on flaky gate", "chose JSONL for the memory store"} {
		if !strings.Contains(got, want) {
			t.Errorf("prompt missing memory detail %q; got: %.500s", want, got)
		}
	}
}

// Cold start: a repo with NO memory store must build a prompt without error and
// without a memory block — absence is the normal first-run case, never a failure.
func TestBuildPrompt_MissingMemoryIsColdStart(t *testing.T) {
	root := t.TempDir() // no .forge/memory.jsonl
	got := buildPrompt(root, asset.Phase{Name: "build", Agent: "implementer"}, "balanced")
	if strings.Contains(got, "Project memory") {
		t.Errorf("cold start must omit the memory block; got: %.300s", got)
	}
	if strings.Contains(got, "UNREADABLE") {
		t.Errorf("a missing store must not be reported as unreadable; got: %.300s", got)
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

// --- resilience wiring: timeout / checkpoint / resume / trace -----------------

// fakeRepo builds a self-contained repo root in a temp dir with the bits the CLI
// needs to load a workflow without the real ForgeOS tree: a stub yaml2json.py
// that emits the given workflow JSON, the workflow yml (content unused — the stub
// ignores it), and an empty .agent/agents dir. It returns the root.
func fakeRepo(t *testing.T, name, workflowJSON string) string {
	t.Helper()
	root := t.TempDir()
	mkdir(t, filepath.Join(root, "harness"))
	mkdir(t, filepath.Join(root, ".agent", "workflows"))
	mkdir(t, filepath.Join(root, ".agent", "agents"))
	// The stub transcoder ignores its argument and prints the workflow JSON, so
	// loadWorkflow's `python3 yaml2json.py <yml>` yields our fixture deterministically.
	shim := "import sys\nsys.stdout.write(" + pyQuote(workflowJSON) + ")\n"
	writeFile(t, filepath.Join(root, "harness", "yaml2json.py"), shim)
	writeFile(t, filepath.Join(root, ".agent", "workflows", name+".yml"), "stub: true\n")
	return root
}

// externalAgentWorkflow is an external-stop workflow with a single agent phase
// and NO gate phases, so the dry executor runs it with no node/harness probe and
// it reaches the safety bound cleanly (the expected external-stop outcome).
const externalAgentWorkflow = `{
  "stage": "evolve",
  "phases": [{"name": "implementer", "agent": "implementer", "readonly": false, "required_gates": []}],
  "stop_condition": {"type": "external", "all_of": [], "anti_pattern": "round_count"}
}`

// --timeout must parse on both run and evolve (DurationVar): a bad duration is a
// parse error (exit 2), a good one is accepted and the command proceeds.
func TestEvolve_TimeoutFlagParses(t *testing.T) {
	if _, err := exec.LookPath("python3"); err != nil {
		t.Skip("python3 not available")
	}
	root := fakeRepo(t, "evolve", externalAgentWorkflow)
	// Valid duration + dry executor + max-iter 1 => one clean external-stop iter.
	code := cmdEvolve([]string{"evolve", "--root", root, "--timeout", "30s", "--max-iter", "1"})
	if code != 0 {
		t.Errorf("evolve --timeout 30s should run an external-stop loop to a clean stop; exit=%d", code)
	}
	// A malformed duration must be rejected at flag parse (exit 2), not ignored.
	if code := cmdEvolve([]string{"evolve", "--root", root, "--timeout", "notaduration"}); code != 2 {
		t.Errorf("malformed --timeout must be a parse error; exit=%d, want 2", code)
	}
}

// --max-retries must parse on both run and evolve (IntVar): a valid value is
// accepted and the command proceeds to a clean stop; a non-integer is a parse
// error (exit 2), not silently ignored. The default (omitted) is 0 == no retries.
func TestMaxRetriesFlagParses(t *testing.T) {
	if _, err := exec.LookPath("python3"); err != nil {
		t.Skip("python3 not available")
	}
	root := fakeRepo(t, "evolve", externalAgentWorkflow)
	// Valid value on evolve: a clean external-stop loop (dry executor never errors).
	if code := cmdEvolve([]string{"evolve", "--root", root, "--max-retries", "2", "--max-iter", "1"}); code != 0 {
		t.Errorf("evolve --max-retries 2 should run to a clean stop; exit=%d", code)
	}
	// Valid value on run: a single agent phase, dry executor, exits 0.
	runRoot := fakeRepo(t, "build", externalAgentWorkflow)
	if code := cmdRun([]string{"build", "--root", runRoot, "--max-retries", "3"}); code != 0 {
		t.Errorf("run --max-retries 3 should complete cleanly; exit=%d", code)
	}
	// A non-integer must be rejected at flag parse (exit 2), never ignored.
	if code := cmdEvolve([]string{"evolve", "--root", root, "--max-retries", "notanint"}); code != 2 {
		t.Errorf("malformed --max-retries must be a parse error; exit=%d, want 2", code)
	}
}

// A full evolve run must materialize <root>/.forge with a checkpoint at the last
// iteration and a non-empty trace; a follow-up --resume must then continue from
// the persisted iteration+1 (proving the write->resume round-trip end to end).
func TestEvolve_WritesCheckpointAndResumes(t *testing.T) {
	if _, err := exec.LookPath("python3"); err != nil {
		t.Skip("python3 not available")
	}
	root := fakeRepo(t, "evolve", externalAgentWorkflow)
	if code := cmdEvolve([]string{"evolve", "--root", root, "--max-iter", "3"}); code != 0 {
		t.Fatalf("first evolve run exit=%d, want 0 (external stop)", code)
	}
	cp, found, err := persist.Load(checkpointPath(root))
	if err != nil || !found {
		t.Fatalf("checkpoint after run: found=%v err=%v", found, err)
	}
	if cp.Iteration != 3 || cp.Workflow != "evolve" {
		t.Errorf("checkpoint = %+v, want last iteration 3 of workflow evolve", cp)
	}
	if info, err := os.Stat(filepath.Join(root, ".forge", "trace.jsonl")); err != nil || info.Size() == 0 {
		t.Errorf("trace.jsonl should exist and be non-empty; err=%v", err)
	}
	// Resume must pick up at cp.Iteration+1 (=4). With max-iter 3 there is nothing
	// left to do, so it ends immediately at the bound without rerunning 1..3.
	if code := cmdEvolve([]string{"evolve", "--root", root, "--max-iter", "3", "--resume"}); code != 0 {
		t.Errorf("resume run exit=%d, want 0", code)
	}
}

// resumeStart is the fail-closed gate for --resume. Three paths: no --resume is a
// fresh run; a missing checkpoint with --resume is tolerated (fresh, reported); a
// MALFORMED checkpoint with --resume is a hard error — never a silent restart.
func TestResumeStart_Paths(t *testing.T) {
	root := t.TempDir()

	// No --resume: fresh sentinel, no IO, no error.
	if start, prev, err := resumeStart(root, false); err != nil || start != 0 || prev != -1.0 {
		t.Errorf("no-resume = (%d,%v,%v), want (0,-1,nil)", start, prev, err)
	}
	// --resume, no checkpoint file present: tolerated as a fresh start.
	if start, prev, err := resumeStart(root, true); err != nil || start != 0 || prev != -1.0 {
		t.Errorf("resume+missing = (%d,%v,%v), want (0,-1,nil)", start, prev, err)
	}
	// --resume with a present, valid checkpoint: continue at Iteration+1, seed prev.
	cp := persist.Checkpoint{Workflow: "evolve", Iteration: 5, RoadmapCompletion: 0.6}
	if err := persist.Save(checkpointPath(root), cp); err != nil {
		t.Fatalf("seed checkpoint: %v", err)
	}
	if start, prev, err := resumeStart(root, true); err != nil || start != 6 || prev != 0.6 {
		t.Errorf("resume+valid = (%d,%v,%v), want (6,0.6,nil)", start, prev, err)
	}
	// --resume with a MALFORMED checkpoint: hard error, no silent from-scratch.
	writeFile(t, checkpointPath(root), "{not valid json")
	if start, _, err := resumeStart(root, true); err == nil || start != 0 {
		t.Errorf("resume+malformed must error out (got start=%d err=%v)", start, err)
	}
}

// checkpointHook is the per-iteration persistence+trace point. Invoking the
// returned closure must write the iteration's snapshot to <root>/.forge and emit
// a matching "iteration" trace event carrying the measured signals.
func TestCheckpointHook_PersistsAndTraces(t *testing.T) {
	root := t.TempDir()
	mkdir(t, filepath.Join(root, ".forge"))
	var buf bytes.Buffer
	var logs []string
	o := runOpts{root: root, mode: "balanced"}
	wf := asset.Workflow{Stage: "evolve"}
	hook := checkpointHook(o, wf, trace.NewTracer(&buf), func(s string) { logs = append(logs, s) })

	hook(2, converge.Signals{RoadmapCompletion: 0.75, GatesGreen: true})

	cp, found, err := persist.Load(checkpointPath(root))
	if err != nil || !found {
		t.Fatalf("checkpoint: found=%v err=%v", found, err)
	}
	if cp.Iteration != 2 || cp.RoadmapCompletion != 0.75 || !cp.GatesGreen || cp.Mode != "balanced" {
		t.Errorf("checkpoint = %+v, want iter 2 / 0.75 / green / balanced", cp)
	}
	if cp.UpdatedAtUnix == 0 || cp.UpdatedAtUnix > time.Now().Unix()+5 {
		t.Errorf("UpdatedAtUnix = %d, want a recent main-injected timestamp", cp.UpdatedAtUnix)
	}
	ev := lastTraceEvent(t, buf.String())
	if ev.Kind != "iteration" || ev.Name != "2" || ev.Status != "ok" {
		t.Errorf("trace event = %+v, want iteration/2/ok", ev)
	}
	if !strings.Contains(ev.Detail, "roadmap=75%") {
		t.Errorf("trace detail = %q, want measured signals", ev.Detail)
	}
}

// The OnIteration hook must ALSO append one memory entry per round — the run
// trajectory recorded for cross-session recall. It is a KindLesson on the stage
// topic, carrying this round's measured signals and iteration number. (Honesty:
// this records the real dry-run trajectory, not a fabricated agent finding.)
func TestCheckpointHook_AppendsMemoryEntry(t *testing.T) {
	root := t.TempDir()
	mkdir(t, filepath.Join(root, ".forge"))
	var buf bytes.Buffer
	hook := checkpointHook(runOpts{root: root, mode: "balanced"}, asset.Workflow{Stage: "evolve"},
		trace.NewTracer(&buf), func(string) {})

	hook(3, converge.Signals{RoadmapCompletion: 0.4, GatesGreen: false})

	entries, err := memory.Load(memoryPath(root))
	if err != nil {
		t.Fatalf("load memory: %v", err)
	}
	if len(entries) != 1 {
		t.Fatalf("hook should append exactly one memory entry, got %d", len(entries))
	}
	e := entries[0]
	if e.Kind != memory.KindLesson || e.Topic != "evolve" || e.Iteration != 3 {
		t.Errorf("entry = %+v, want KindLesson / topic evolve / iter 3", e)
	}
	if !strings.Contains(e.Detail, "roadmap=40%") || !strings.Contains(e.Detail, "gates_green=false") {
		t.Errorf("entry detail = %q, want the round's measured trajectory", e.Detail)
	}
	if e.CreatedAtUnix == 0 {
		t.Errorf("entry must carry a main-injected timestamp; got %d", e.CreatedAtUnix)
	}
}

// Fail-closed honesty: when the checkpoint write fails, the loop must NOT pretend
// it succeeded — the trace event status flips to a failure marker and a loud
// warning is logged, rather than silently dropping the recovery state.
func TestCheckpointHook_WriteFailureIsSurfaced(t *testing.T) {
	root := t.TempDir()
	// Make checkpointPath unwritable by planting a DIRECTORY where the file (and
	// its .tmp sibling) must go, so persist.Save's open/rename fails.
	mkdir(t, filepath.Join(root, ".forge"))
	mkdir(t, checkpointPath(root))
	var buf bytes.Buffer
	var logs []string
	hook := checkpointHook(runOpts{root: root}, asset.Workflow{Stage: "evolve"},
		trace.NewTracer(&buf), func(s string) { logs = append(logs, s) })

	hook(1, converge.Signals{RoadmapCompletion: 0.1})

	ev := lastTraceEvent(t, buf.String())
	if ev.Status != "checkpoint-write-failed" {
		t.Errorf("trace status = %q, want checkpoint-write-failed on a failed Save", ev.Status)
	}
	if !containsSub(logs, "checkpoint write failed") {
		t.Errorf("a failed checkpoint write must be logged loudly; logs=%v", logs)
	}
}

// openTracer must create <root>/.forge and APPEND to trace.jsonl across calls, so
// a --resume continues the same audit trail instead of truncating prior history.
func TestOpenTracer_CreatesDirAndAppends(t *testing.T) {
	root := t.TempDir()
	tr, closeFn, err := openTracer(root)
	if err != nil {
		t.Fatalf("openTracer: %v", err)
	}
	if err := tr.Emit(trace.Event{Kind: "iteration", Name: "1", Status: "ok"}); err != nil {
		t.Fatalf("emit: %v", err)
	}
	closeFn()
	// Second open (simulating a resume) must append, not truncate.
	tr2, closeFn2, err := openTracer(root)
	if err != nil {
		t.Fatalf("openTracer #2: %v", err)
	}
	if err := tr2.Emit(trace.Event{Kind: "iteration", Name: "2", Status: "ok"}); err != nil {
		t.Fatalf("emit #2: %v", err)
	}
	closeFn2()
	data, err := os.ReadFile(filepath.Join(root, ".forge", "trace.jsonl"))
	if err != nil {
		t.Fatalf("read trace: %v", err)
	}
	if n := bytes.Count(data, []byte("\n")); n != 2 {
		t.Errorf("trace has %d lines, want 2 (append preserved the first across reopen)", n)
	}
}

// --- test helpers ------------------------------------------------------------

func mkdir(t *testing.T, dir string) {
	t.Helper()
	if err := os.MkdirAll(dir, 0o755); err != nil {
		t.Fatalf("mkdir %s: %v", dir, err)
	}
}

func writeFile(t *testing.T, path, content string) {
	t.Helper()
	if err := os.WriteFile(path, []byte(content), 0o644); err != nil {
		t.Fatalf("write %s: %v", path, err)
	}
}

// pyQuote renders s as a Python string literal (JSON's quoting is a compatible
// subset for our ASCII workflow fixtures), so the stub transcoder can emit it.
func pyQuote(s string) string {
	b, _ := json.Marshal(s)
	return string(b)
}

// lastTraceEvent decodes the final JSONL record from a trace buffer.
func lastTraceEvent(t *testing.T, jsonl string) trace.Event {
	t.Helper()
	lines := strings.Split(strings.TrimRight(jsonl, "\n"), "\n")
	if len(lines) == 0 || lines[0] == "" {
		t.Fatalf("no trace events emitted; buf=%q", jsonl)
	}
	var ev trace.Event
	if err := json.Unmarshal([]byte(lines[len(lines)-1]), &ev); err != nil {
		t.Fatalf("decode trace line: %v", err)
	}
	return ev
}

func containsSub(ss []string, sub string) bool {
	for _, s := range ss {
		if strings.Contains(s, sub) {
			return true
		}
	}
	return false
}
