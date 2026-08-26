package main

import (
	"encoding/json"
	"fmt"
	"io/fs"
	"os"
	"path/filepath"
	"strings"
	"testing"

	"forgeos/forge-core/internal/artifact"
)

// TestShippedWorkflowsFakeAgentDeliveryLifecycle exercises the real seven
// workflow assets without a network, model, cloud credential, or remote
// deployment. A deterministic executable named "claude" is used only as the
// command-executor seam: it writes each phase's declared local artifact and
// returns the machine contract that phase expects.
func TestShippedWorkflowsFakeAgentDeliveryLifecycle(t *testing.T) {
	if repoRoot() == "" {
		t.Skip("ForgeOS repository root unavailable")
	}
	root, fake := shippedWorkflowFixture(t)
	t.Setenv("USER", "forge-e2e-test")
	test := shippedWorkflowE2E{root: root, fake: fake}
	test.discover(t)
	waiting := test.waitForDeploy(t)
	test.rejectDeploy(t, waiting)
	test.approveDeployAndEvolve(t, waiting)
	test.rollback(t)
}

func TestShippedBoundBuildNotConvergedPersistsExactRecoveryState(t *testing.T) {
	if repoRoot() == "" {
		t.Skip("ForgeOS repository root unavailable")
	}
	root, fake := shippedWorkflowFixture(t)
	t.Setenv("USER", "forge-e2e-test")
	test := shippedWorkflowE2E{root: root, fake: fake}
	test.discover(t)
	reopenFixtureRoadmap(t, root)

	code, out := advanceShippedChainToDeploy(t, root, fake, "engineering")
	if code != exitChainIncomplete || !strings.Contains(out, "stage=build (NOT MET") ||
		strings.Contains(out, "persistence failed") {
		t.Fatalf("unmet bound Build exit=%d; output:\n%s", code, out)
	}
	state := mustChainState(t, root)
	if state.Status != "not_converged" || state.CurrentStage != "build" ||
		strings.Join(state.CompletedStages, ",") != "design,review" {
		t.Fatalf("unmet bound Build state = %+v", state)
	}
	assertIncompleteStageRecoveryAbsent(t, state, "build")
	if state.StageReceipts["design"] == "" || state.StageReceipts["review"] == "" {
		t.Fatal("unmet Build discarded completed-stage recovery references")
	}
	if err := validateBoundChainRecovery(root, state); err != nil {
		t.Fatalf("persisted unmet Build recovery is invalid: %v", err)
	}
}

type shippedWorkflowE2E struct {
	root string
	fake string
}

func (e shippedWorkflowE2E) discover(t *testing.T) {
	t.Helper()
	code, out := captureChainOutput(t, func() int {
		return cmdRun(shippedRunArgs("discover", e.root, e.fake, false, false, "engineering"))
	})
	if code != 0 || !strings.Contains(out, "convergence: MET") {
		t.Fatalf("real discover workflow exit=%d, want MET; output:\n%s", code, out)
	}
	assertFilesNonEmpty(t, e.root, []string{
		"docs/discovery/requirement-draft.md",
		"docs/discovery/capability-matrix.md",
		"docs/discovery/citations.md",
		"docs/discovery/prd.md",
	})
}

func (e shippedWorkflowE2E) waitForDeploy(t *testing.T) chainState {
	t.Helper()
	code, out := advanceShippedChainToDeploy(t, e.root, e.fake, "engineering")
	if code != exitChainIncomplete ||
		!strings.Contains(out, "waiting for human approval at stage=deploy") {
		t.Fatalf("real spine first pass exit=%d, want deploy wait; output:\n%s", code, out)
	}
	waiting := mustChainState(t, e.root)
	if waiting.CurrentStage != "deploy" ||
		strings.Join(waiting.CompletedStages, ",") != "design,review,build" ||
		waiting.RunID == "" {
		t.Fatalf("spine waiting state = %+v", waiting)
	}
	assertFilesNonEmpty(t, e.root, releaseApprovalFiles["deploy"])
	assertSingleDesignADR(t, e.root)
	return waiting
}

func advanceShippedChainToDeploy(t *testing.T, root, fake, selectedMode string) (int, string) {
	t.Helper()
	code, output := captureChainOutput(t, func() int {
		return cmdRun(shippedRunArgs("design", root, fake, true, false, selectedMode))
	})
	if code != exitChainIncomplete || !strings.Contains(output, "approval at stage=design") {
		t.Fatalf("bound Design did not stop for v3 approval: exit=%d\n%s", code, output)
	}
	captureStdout(t, func() {
		if code := writeApproval(root, "design", true); code != 0 {
			t.Fatalf("approve design = %d", code)
		}
	})
	return captureChainOutput(t, func() int {
		return cmdRun(shippedRunArgs("design", root, fake, true, false, selectedMode))
	})
}

func (e shippedWorkflowE2E) rejectDeploy(t *testing.T, waiting chainState) {
	t.Helper()
	captureStdout(t, func() {
		if code := writeApproval(e.root, "deploy", false); code != 0 {
			t.Fatalf("reject deploy = %d", code)
		}
	})
	code, out := captureChainOutput(t, func() int {
		return cmdRun(shippedRunArgs("design", e.root, e.fake, true, false, "engineering"))
	})
	if code != exitChainIncomplete || !strings.Contains(out, "marker consumed") {
		t.Fatalf("deploy rejection pass exit=%d; output:\n%s", code, out)
	}
	if _, err := os.Stat(rejectionPath(e.root, "deploy")); !os.IsNotExist(err) {
		t.Fatalf("deploy rejection marker was not consumed: %v", err)
	}
	afterReject := mustChainState(t, e.root)
	if afterReject.RunID != waiting.RunID ||
		strings.Join(afterReject.CompletedStages, ",") != "design,review,build" {
		t.Fatalf("rejection did not retain the same chain cursor: %+v", afterReject)
	}
}

func (e shippedWorkflowE2E) approveDeployAndEvolve(t *testing.T, waiting chainState) {
	t.Helper()
	captureStdout(t, func() {
		if code := writeApproval(e.root, "deploy", true); code != 0 {
			t.Fatalf("approve deploy = %d", code)
		}
	})
	code, out := captureChainOutput(t, func() int {
		return cmdRun(shippedRunArgs("design", e.root, e.fake, true, false, "engineering"))
	})
	if code != 0 || !strings.Contains(out, "entering evolve LoopEngine") {
		t.Fatalf("approved deploy did not enter evolve: exit=%d\n%s", code, out)
	}
	completed := mustChainState(t, e.root)
	if completed.RunID != waiting.RunID ||
		strings.Join(completed.CompletedStages, ",") != "design,review,build,deploy,evolve" {
		t.Fatalf("completed spine state = %+v", completed)
	}
	assertPhaseCallCount(t, e.root, "release-planning", 2)
	assertPhaseCallCount(t, e.root, "release-plan-validation", 2)
}

func (e shippedWorkflowE2E) rollback(t *testing.T) {
	t.Helper()
	code, out := captureChainOutput(t, func() int {
		return cmdRun(shippedRunArgs("rollback", e.root, e.fake, true, false, "engineering"))
	})
	if code != exitChainIncomplete ||
		!strings.Contains(out, "waiting for human approval at stage=rollback") {
		t.Fatalf("rollback first pass exit=%d; output:\n%s", code, out)
	}
	rollbackWaiting := mustChainState(t, e.root)
	e.rejectAndApproveRollback(t)
	rollbackDone := mustChainState(t, e.root)
	if rollbackDone.RunID != rollbackWaiting.RunID ||
		strings.Join(rollbackDone.CompletedStages, ",") != "rollback" {
		t.Fatalf("rollback completion state = %+v", rollbackDone)
	}
	assertFilesNonEmpty(t, e.root, releaseApprovalFiles["rollback"])
	assertPhaseCallCount(t, e.root, "rollback-planning", 2)
	assertPhaseCallCount(t, e.root, "rollback-plan-validation", 2)
}

func (e shippedWorkflowE2E) rejectAndApproveRollback(t *testing.T) {
	t.Helper()
	captureStdout(t, func() {
		if code := writeApproval(e.root, "rollback", false); code != 0 {
			t.Fatalf("reject rollback = %d", code)
		}
	})
	code, out := captureChainOutput(t, func() int {
		return cmdRun(shippedRunArgs("rollback", e.root, e.fake, true, false, "engineering"))
	})
	if code != exitChainIncomplete || !strings.Contains(out, "marker consumed") {
		t.Fatalf("rollback rejection pass exit=%d; output:\n%s", code, out)
	}
	captureStdout(t, func() {
		if code := writeApproval(e.root, "rollback", true); code != 0 {
			t.Fatalf("approve rollback = %d", code)
		}
	})
	code, out = captureChainOutput(t, func() int {
		return cmdRun(shippedRunArgs("rollback", e.root, e.fake, true, false, "engineering"))
	})
	if code != 0 {
		t.Fatalf("approved rollback exit=%d; output:\n%s", code, out)
	}
}

func shippedRunArgs(stage, root, fake string, chain, approved bool, modes ...string) []string {
	mode := "balanced"
	if len(modes) > 0 {
		mode = modes[0]
	}
	digest, err := fileSHA256(fake)
	if err != nil {
		panic(fmt.Sprintf("hash fake claude: %v", err))
	}
	args := []string{
		stage,
		"--root", root,
		"--executor", "command",
		"--agent-cmd", filepath.Base(fake),
		"--release-agent-path", fake,
		"--release-agent-sha256", digest,
		"--mode", mode,
		"--lifecycle", "idea",
		"--max-agent-calls", "100",
		"--max-chain-stages", "8",
	}
	if chain {
		args = append(args, "--chain")
	}
	if approved {
		args = append(args, "--approved")
	}
	return args
}

func shippedWorkflowFixture(t *testing.T) (string, string) {
	t.Helper()
	source := repoRoot()
	root := t.TempDir()
	for _, rel := range []string{
		".agent/workflows", ".agent/agents", ".agent/policies",
		".ai/prompts", "docs/adr", "docs/release",
	} {
		copyFixtureTree(t, source, root, rel)
	}
	for _, rel := range []string{
		".agent/AGENTS.md", ".agent/ROADMAP.md", ".agent/project.yml",
	} {
		copyFixtureFile(t, source, root, rel)
	}
	completeFixtureRoadmap(t, root)
	writeHarnessStubs(t, root)
	fake := writeFakeClaude(t)
	t.Setenv("PATH", filepath.Dir(fake)+string(os.PathListSeparator)+os.Getenv("PATH"))
	if err := os.WriteFile(filepath.Join(root, ".gitignore"), []byte(".forge/\n"), 0o600); err != nil {
		t.Fatal(err)
	}
	mustGit(t, root, "init", "-q")
	mustGit(t, root, "config", "user.name", "Forge E2E")
	mustGit(t, root, "config", "user.email", "forge-e2e@example.invalid")
	mustGit(t, root, "add", ".")
	mustGit(t, root, "commit", "-q", "-m", "shipped workflow fixture")
	return root, fake
}

func completeFixtureRoadmap(t *testing.T, root string) {
	t.Helper()
	path := filepath.Join(root, ".agent", "ROADMAP.md")
	data, err := os.ReadFile(path)
	if err != nil {
		t.Fatal(err)
	}
	completed := strings.ReplaceAll(string(data), "- [ ]", "- [x]")
	if err := os.WriteFile(path, []byte(completed), 0o600); err != nil {
		t.Fatal(err)
	}
}

func reopenFixtureRoadmap(t *testing.T, root string) {
	t.Helper()
	path := filepath.Join(root, ".agent", "ROADMAP.md")
	data, err := os.ReadFile(path)
	if err != nil {
		t.Fatal(err)
	}
	open := strings.Replace(string(data), "- [x]", "- [ ]", 1)
	if open == string(data) {
		t.Fatal("fixture Roadmap has no completed item to reopen")
	}
	if err := os.WriteFile(path, []byte(open), 0o600); err != nil {
		t.Fatal(err)
	}
}

func assertIncompleteStageRecoveryAbsent(t *testing.T, state chainState, stage string) {
	t.Helper()
	for key := range state.PhaseReceipts {
		if strings.HasPrefix(key, stage+"/") {
			t.Fatalf("incomplete stage retained phase receipt reference %q", key)
		}
	}
	if state.StageReceipts[stage] != "" || state.ApprovalContexts[stage] != "" {
		t.Fatalf("incomplete stage retained stage recovery references")
	}
}

func copyFixtureTree(t *testing.T, source, target, relative string) {
	t.Helper()
	base := filepath.Join(source, relative)
	if err := filepath.WalkDir(base, func(path string, entry fs.DirEntry, err error) error {
		if err != nil {
			return err
		}
		rel, err := filepath.Rel(source, path)
		if err != nil {
			return err
		}
		dst := filepath.Join(target, rel)
		if entry.IsDir() {
			return os.MkdirAll(dst, 0o755)
		}
		if entry.Type().IsRegular() {
			data, err := os.ReadFile(path)
			if err != nil {
				return err
			}
			return os.WriteFile(dst, data, 0o600)
		}
		return nil
	}); err != nil {
		t.Fatalf("copy fixture tree %s: %v", relative, err)
	}
}

func copyFixtureFile(t *testing.T, source, target, relative string) {
	t.Helper()
	data, err := os.ReadFile(filepath.Join(source, relative))
	if err != nil {
		t.Fatalf("read fixture %s: %v", relative, err)
	}
	path := filepath.Join(target, relative)
	if err := os.MkdirAll(filepath.Dir(path), 0o755); err != nil {
		t.Fatal(err)
	}
	if err := os.WriteFile(path, data, 0o600); err != nil {
		t.Fatal(err)
	}
}

func writeHarnessStubs(t *testing.T, root string) {
	t.Helper()
	dir := filepath.Join(root, "harness")
	if err := os.MkdirAll(dir, 0o755); err != nil {
		t.Fatal(err)
	}
	acceptance := `#!/usr/bin/env node
const criteria = [
  'test_pass', 'app_test_pass', 'complexity_violations', 'arch_violations',
  'architecture', 'security_findings', 'dependency_vulnerabilities', 'lint',
  'typecheck', 'build', 'coverage',
];
const rows = criteria.map((criterion) => ({
  criterion, status: 'PASS', detail: 'deterministic fake gate', category: 'applicable',
}));
if (process.argv.includes('--json')) console.log(JSON.stringify(rows));
else console.log('forge accept: ACCEPTED (deterministic fixture)');
`
	if err := os.WriteFile(filepath.Join(dir, "acceptance.mjs"), []byte(acceptance), 0o600); err != nil {
		t.Fatal(err)
	}
	if err := os.WriteFile(
		filepath.Join(dir, "gate.mjs"),
		[]byte("#!/usr/bin/env node\nconsole.log('forge-gate: PASS (deterministic fixture)');\n"),
		0o600,
	); err != nil {
		t.Fatal(err)
	}
	if err := os.WriteFile(
		filepath.Join(dir, "check.py"),
		[]byte("print('forge-check: PASS (deterministic fixture)')\n"),
		0o600,
	); err != nil {
		t.Fatal(err)
	}
}

const fakeClaudeScript = "VERDICT: APPROVE"

func writeFakeClaude(t *testing.T) string {
	t.Helper()
	path := filepath.Join(t.TempDir(), "claude")
	buildNativeFakeClaude(t, path, fakeClaudeScript)
	return path
}

func assertFilesNonEmpty(t *testing.T, root string, relative []string) {
	t.Helper()
	for _, rel := range relative {
		data, err := os.ReadFile(filepath.Join(root, rel))
		if err != nil || len(strings.TrimSpace(string(data))) == 0 {
			t.Errorf("artifact %s is missing/empty: %v", rel, err)
		}
	}
}

func assertSingleDesignADR(t *testing.T, root string) {
	t.Helper()
	pattern := filepath.Join(root, "docs", "adr", "ADR-*-deterministic-design.md")
	matches, err := filepath.Glob(pattern)
	if err != nil || len(matches) != 1 {
		t.Fatalf("dynamic design ADRs = %v, %v; want exactly one", matches, err)
	}
	records, err := artifact.Load(root)
	if err != nil {
		t.Fatal(err)
	}
	for _, record := range records {
		if record.Workflow == "design" && record.Phase == "solution-architect" &&
			record.Path == "docs/adr/"+filepath.Base(matches[0]) {
			return
		}
	}
	t.Fatalf("dynamic design ADR %s has no provenance record", matches[0])
}

func assertPhaseCallCount(t *testing.T, root, phase string, want int) {
	t.Helper()
	data, err := os.ReadFile(tracePath(root))
	if err != nil {
		t.Fatal(err)
	}
	got := 0
	for _, line := range strings.Split(strings.TrimSpace(string(data)), "\n") {
		var event struct {
			Kind string `json:"kind"`
			Name string `json:"name"`
		}
		if json.Unmarshal([]byte(line), &event) == nil &&
			event.Kind == "agent" && event.Name == phase {
			got++
		}
	}
	if got != want {
		t.Errorf("traced agent phase %s calls = %d, want %d", phase, got, want)
	}
}
