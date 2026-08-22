package main

import (
	"os"
	"path/filepath"
	"strings"
	"testing"
	"time"

	"forgeos/forge-core/internal/asset"
)

func TestReleaseArtifactMustChangeDuringCurrentAttempt(t *testing.T) {
	root := t.TempDir()
	initReleaseTestGit(t, root)
	path := releaseApprovalFiles["deploy"][0]
	writeReleaseTestFile(t, root, path, "stale\n")
	phase := asset.Phase{
		Name: "release-planning", Agent: "release-engineer",
		Emits: []string{path},
	}
	prov := newArtifactProvenance(root, "deploy", "run-1", strings.Repeat("a", 64))
	recordReleaseBuild(t, prov, phase, "sonnet", "prompt")
	if err := prov.appendEmits(phase, phase.Emits); err == nil ||
		!strings.Contains(err.Error(), "not created or content-changed") {
		t.Fatalf("stale emit error = %v", err)
	}
	writeReleaseTestFile(t, root, path, "fresh\n")
	if err := prov.appendEmits(phase, phase.Emits); err != nil {
		t.Fatalf("content-changed emit rejected: %v", err)
	}
}

type releaseTreeMutationCase struct {
	name    string
	prepare func(*testing.T, string, string)
	mutate  func(*testing.T, string, string)
}

var releaseTreeMutationCases = []releaseTreeMutationCase{
	{
		name: "create file",
		mutate: func(t *testing.T, root, extra string) {
			writeReleaseTestFile(t, root, extra, "created\n")
		},
	},
	{
		name: "modify file",
		prepare: func(t *testing.T, root, extra string) {
			writeReleaseTestFile(t, root, extra, "before\n")
		},
		mutate: func(t *testing.T, root, extra string) {
			writeReleaseTestFile(t, root, extra, "after\n")
		},
	},
	{
		name: "rewrite identical bytes",
		prepare: func(t *testing.T, root, extra string) {
			writeReleaseTestFile(t, root, extra, "same\n")
		},
		mutate: func(t *testing.T, root, extra string) {
			path := filepath.Join(root, filepath.FromSlash(extra))
			writeReleaseTestFile(t, root, extra, "same\n")
			if err := os.Chtimes(path, time.Unix(123, 0), time.Unix(123, 0)); err != nil {
				t.Fatal(err)
			}
		},
	},
	{
		name: "delete file",
		prepare: func(t *testing.T, root, extra string) {
			writeReleaseTestFile(t, root, extra, "before\n")
		},
		mutate: func(t *testing.T, root, extra string) {
			if err := os.Remove(filepath.Join(root, filepath.FromSlash(extra))); err != nil {
				t.Fatal(err)
			}
		},
	},
	{
		name: "create directory",
		mutate: func(t *testing.T, root, _ string) {
			if err := os.Mkdir(filepath.Join(root, "docs", "release", "undeclared-dir"), 0o700); err != nil {
				t.Fatal(err)
			}
		},
	},
}

func TestReleasePostflightRejectsEveryUndeclaredTreeDelta(t *testing.T) {
	for _, tc := range releaseTreeMutationCases {
		t.Run(tc.name, func(t *testing.T) {
			root := t.TempDir()
			initReleaseTestGit(t, root)
			emit := releaseApprovalFiles["deploy"][0]
			extra := "docs/release/undeclared.md"
			writeReleaseTestFile(t, root, emit, "stale\n")
			if tc.prepare != nil {
				tc.prepare(t, root, extra)
			}
			phase := asset.Phase{
				Name: "release-planning", Agent: "release-engineer",
				Emits: []string{emit},
			}
			prov := newArtifactProvenance(root, "deploy", "run-1")
			recordReleaseBuild(t, prov, phase, "sonnet", "prompt")
			writeReleaseTestFile(t, root, emit, "fresh\n")
			tc.mutate(t, root, extra)
			err := prov.appendEmits(phase, phase.Emits)
			if err == nil || !strings.Contains(err.Error(), "undeclared release path") {
				t.Fatalf("undeclared release delta error = %v", err)
			}
		})
	}
}

func TestReleaseValidationRequiresMatchingStdoutAndReportVerdict(t *testing.T) {
	root := t.TempDir()
	initReleaseTestGit(t, root)
	report := releaseApprovalFiles["deploy"][4]
	writeReleaseTestFile(t, root, report, "old\n")
	phase := asset.Phase{
		Name: "release-plan-validation", Agent: "release-engineer",
		Emits: []string{report},
	}
	wf := asset.Workflow{Stage: "deploy", Phases: []asset.Phase{phase}}
	prov := newArtifactProvenance(root, "deploy", "run-1", strings.Repeat("a", 64))
	recordReleaseBuild(t, prov, phase, "sonnet", "prompt")
	writeReleaseTestFile(t, root, report, "review\nVERDICT: REQUEST_CHANGES\n")

	err := phaseOutputContract(root, wf, prov)(phase.Name, "VERDICT: APPROVE")
	if err == nil || !strings.Contains(err.Error(), "does not match report verdict") {
		t.Fatalf("mismatched validation verdict error = %v", err)
	}
	if validReleaseValidationReceipt(root, "deploy") {
		t.Fatal("mismatched validation verdict wrote a receipt")
	}
}

func TestReleaseValidationApproveWritesBoundReceipt(t *testing.T) {
	root := t.TempDir()
	initReleaseTestGit(t, root)
	for _, relative := range releaseApprovalFiles["deploy"] {
		writeReleaseTestFile(t, root, relative, "seed\n")
	}
	phase := asset.Phase{
		Name: "release-plan-validation", Agent: "release-engineer",
		Emits: []string{releaseApprovalFiles["deploy"][4]},
	}
	wf := asset.Workflow{Stage: "deploy", Phases: []asset.Phase{phase}}
	prov := newArtifactProvenance(root, "deploy", "run-approve", strings.Repeat("a", 64))
	recordReleaseBuild(t, prov, phase, "sonnet", "minimal release prompt")
	writeReleaseTestFile(t, root, phase.Emits[0], "checked\nVERDICT: APPROVE\n")

	if err := phaseOutputContract(root, wf, prov)(phase.Name, "VERDICT: APPROVE"); err != nil {
		t.Fatalf("approved validation contract: %v", err)
	}
	if err := prov.writeValidationReceipt(phase); err != nil {
		t.Fatalf("commit approved validation receipt: %v", err)
	}
	if err := verifyReleaseValidationReceipt(root, "deploy"); err != nil {
		t.Fatalf("fresh validation receipt rejected: %v", err)
	}
	data, err := os.ReadFile(filepath.Join(forgeDir(root), "deploy.validation.json"))
	if err != nil {
		t.Fatal(err)
	}
	for _, want := range []string{
		`"run_id":"run-approve"`, `"model":"sonnet"`,
		`"agent_executable_sha256":"` + strings.Repeat("a", 64) + `"`,
	} {
		if !strings.Contains(string(data), want) {
			t.Errorf("receipt missing %s: %s", want, data)
		}
	}
}

func TestReleaseRawEnvelopeFailsClosed(t *testing.T) {
	phase := asset.Phase{Name: "release-plan-validation", Agent: "release-engineer"}
	validate := releaseRawOutputContract(asset.Workflow{Stage: "deploy", Phases: []asset.Phase{phase}})
	cases := []string{
		`{"type":"result","subtype":"success","is_error":true,"result":"VERDICT: APPROVE"}`,
		`{"type":"result","subtype":"error_during_execution","is_error":false,"result":"VERDICT: APPROVE"}`,
		`{"result":"VERDICT: APPROVE","is_error":false}`,
		`{"type":"result"`,
	}
	for _, raw := range cases {
		if err := validate(phase.Name, raw); err == nil {
			t.Errorf("unsafe release envelope accepted: %s", raw)
		}
	}
	valid := `{"type":"result","subtype":"success","is_error":false,"result":"VERDICT: APPROVE","total_cost_usd":0}`
	if err := validate(phase.Name, valid); err != nil {
		t.Fatalf("valid release envelope rejected: %v", err)
	}
}

func TestReleasePlanningReceivesValidationReportOnlyOnRequestChangesRetry(t *testing.T) {
	root := t.TempDir()
	initReleaseTestGit(t, root)
	for _, relative := range releaseApprovalFiles["deploy"] {
		content := relative + "\n"
		if relative == releaseApprovalFiles["deploy"][4] {
			content = "VALIDATION_FEEDBACK_SENTINEL\nVERDICT: REQUEST_CHANGES\n"
		}
		writeReleaseTestFile(t, root, relative, content)
	}
	phase := asset.Phase{
		Name: "release-planning", Agent: "release-engineer", Readonly: true,
		Emits: append([]string(nil), releaseApprovalFiles["deploy"][:4]...),
	}
	cache := newReleasePromptCache()
	if err := cache.prepare(root, phase); err != nil {
		t.Fatal(err)
	}
	initial, _, _, ok := cache.build(phase, "balanced", "sonnet")
	if !ok {
		t.Fatal("initial planning prompt unavailable")
	}
	if strings.Contains(initial, "VALIDATION_FEEDBACK_SENTINEL") {
		t.Fatal("stale validation report leaked into an initial planning attempt")
	}
	if err := cache.prepare(root, phase, true); err != nil {
		t.Fatal(err)
	}
	rework, _, _, ok := cache.build(phase, "balanced", "sonnet")
	if !ok {
		t.Fatal("rework planning prompt unavailable")
	}
	for _, want := range []string{
		"Prior validation report requiring planning changes",
		"[release-input:docs/release/deployment-validation.md]",
		"VALIDATION_FEEDBACK_SENTINEL",
		"VERDICT: REQUEST_CHANGES",
	} {
		if !strings.Contains(rework, want) {
			t.Errorf("rework planning prompt missing %q:\n%s", want, rework)
		}
	}
}

func TestReleaseSourceDigestExcludesGeneratedReleaseDocuments(t *testing.T) {
	root := t.TempDir()
	initReleaseTestGit(t, root)
	before, err := sourceStateRevision(root)
	if err != nil {
		t.Fatal(err)
	}
	writeReleaseTestFile(t, root, "docs/release/transient.md", "generated\n")
	after, err := sourceStateRevision(root)
	if err != nil {
		t.Fatal(err)
	}
	if after != before {
		t.Fatalf("generated release document changed product digest:\n%s\n%s", before, after)
	}
	mustGit(t, root, "add", "docs/release/transient.md")
	mustGit(t, root, "commit", "-q", "-m", "release docs only")
	committed, err := sourceStateRevision(root)
	if err != nil {
		t.Fatal(err)
	}
	if committed != before {
		t.Fatalf("release-docs-only commit changed product digest:\n%s\n%s", before, committed)
	}
}

func TestReleaseStageDoesNotExecuteRepositoryAcceptanceHarness(t *testing.T) {
	root, fake := shippedWorkflowFixture(t)
	writeReleaseTestFile(t, root, "docs/release/release-manifest.yml", "version: unresolved\n")
	sentinel := filepath.Join(root, "acceptance-was-run")
	trap := "#!/usr/bin/env node\nrequire('fs').writeFileSync(" +
		quoteJSString(sentinel) + ", 'ran'); process.exit(99);\n"
	if err := os.WriteFile(filepath.Join(root, "harness", "acceptance.mjs"), []byte(trap), 0o700); err != nil {
		t.Fatal(err)
	}
	code, output := captureChainOutput(t, func() int {
		return cmdRun(shippedRunArgs("rollback", root, fake, true, false))
	})
	if code != 1 || !strings.Contains(output, "cross-stage release input") {
		t.Fatalf("standalone rollback without chain-v5 provenance exit=%d:\n%s", code, output)
	}
	if _, err := os.Stat(sentinel); !os.IsNotExist(err) {
		t.Fatalf("release stage executed repository acceptance harness: %v", err)
	}
}

func TestReleaseMalformedVerdictCannotReachApproval(t *testing.T) {
	root, fake := shippedWorkflowFixture(t)
	rewriteFakeClaude(t, fake, strings.ReplaceAll(fakeClaudeScript, "VERDICT: APPROVE", "VERDICT: MAYBE"))
	code, output := captureChainOutput(t, func() int {
		return cmdRun(shippedRunArgs("deploy", root, fake, true, false))
	})
	if code != 1 || !strings.Contains(output, "no exact binary verdict") {
		t.Fatalf("malformed release verdict exit=%d:\n%s", code, output)
	}
	if validReleaseValidationReceipt(root, "deploy") {
		t.Fatal("malformed release verdict wrote a validation receipt")
	}
	if code := writeApproval(root, "deploy", true); code == 0 {
		t.Fatal("malformed validation was approvable")
	}
}

func TestBoundDeployApproveReceiptReferencesTerminalContext(t *testing.T) {
	root, fake := shippedWorkflowFixture(t)
	code, output := captureChainOutput(t, func() int {
		return cmdRun(shippedRunArgs("deploy", root, fake, true, false))
	})
	if code != exitChainIncomplete || !strings.Contains(output, "waiting for human approval") {
		t.Fatalf("bound deploy wait exit=%d:\n%s", code, output)
	}
	if err := verifyBoundReleaseValidationReceipt(root, "deploy"); err != nil {
		t.Fatalf("bound v2 validation receipt rejected: %v", err)
	}
	path, _ := releaseValidationReceiptPath(root, "deploy")
	data, err := os.ReadFile(path)
	if err != nil || data[len(data)-1] == '\n' {
		t.Fatalf("bound validation wire = %q, %v", data, err)
	}
	receipt, err := decodeBoundReleaseValidationReceipt(data)
	if err != nil || receipt.Format != releaseValidationReceiptFormatV2 ||
		receipt.AgentOutputReceiptSHA256 == "" || receipt.ApprovalContextSHA256 == "" {
		t.Fatalf("bound validation receipt = %#v, %v", receipt, err)
	}
}

func TestReleasePersistentRequestChangesExhaustionFailsClosed(t *testing.T) {
	root, fake := shippedWorkflowFixture(t)
	script := strings.ReplaceAll(fakeClaudeScript, "VERDICT: APPROVE", "VERDICT: REQUEST_CHANGES")
	rewriteFakeClaude(t, fake, script)
	code, output := captureChainOutput(t, func() int {
		return cmdRun(shippedRunArgs("deploy", root, fake, true, false))
	})
	if code != 1 || !strings.Contains(output, "could not take its required directed loop-back") {
		t.Fatalf("persistent REQUEST_CHANGES exit=%d:\n%s", code, output)
	}
	assertPhaseCallCount(t, root, "release-plan-validation", maxLoopBack+1)
	if validReleaseValidationReceipt(root, "deploy") {
		t.Fatal("exhausted REQUEST_CHANGES wrote a validation receipt")
	}
	if code := writeApproval(root, "deploy", true); code == 0 {
		t.Fatal("exhausted REQUEST_CHANGES was approvable")
	}
}

func TestRejectedReleaseFailureKeepsDurableWaitingCursor(t *testing.T) {
	root, fake := shippedWorkflowFixture(t)
	code, output := advanceShippedChainToDeploy(t, root, fake, "balanced")
	if code != exitChainIncomplete || !strings.Contains(output, "stage=deploy") {
		t.Fatalf("initial deploy wait exit=%d:\n%s", code, output)
	}
	captureStdout(t, func() {
		if code := writeApproval(root, "deploy", false); code != 0 {
			t.Fatalf("reject deploy = %d", code)
		}
	})
	rewriteFakeClaude(t, fake, strings.ReplaceAll(fakeClaudeScript, "VERDICT: APPROVE", "VERDICT: MAYBE"))
	code, output = captureChainOutput(t, func() int {
		return cmdRun(shippedRunArgs("design", root, fake, true, false))
	})
	if code != 1 || !strings.Contains(output, "marker retained for retry") {
		t.Fatalf("failed rejected rework exit=%d:\n%s", code, output)
	}
	state := mustChainState(t, root)
	if state.Status != "waiting_approval" || state.CurrentStage != "deploy" {
		t.Fatalf("failed rejected rework lost waiting cursor: %+v", state)
	}
	if _, err := os.Stat(rejectionPath(root, "deploy")); err != nil {
		t.Fatalf("failed rejected rework consumed marker: %v", err)
	}

	rewriteFakeClaude(t, fake, fakeClaudeScript)
	code, output = captureChainOutput(t, func() int {
		return cmdRun(shippedRunArgs("design", root, fake, true, false))
	})
	if code != exitChainIncomplete || !strings.Contains(output, "marker consumed") {
		t.Fatalf("retry after rejected failure exit=%d:\n%s", code, output)
	}
	if _, err := os.Stat(rejectionPath(root, "deploy")); !os.IsNotExist(err) {
		t.Fatalf("successful retry did not consume rejection marker: %v", err)
	}
}

func writeReleaseTestFile(t *testing.T, root, relative, content string) {
	t.Helper()
	path := filepath.Join(root, filepath.FromSlash(relative))
	if err := os.MkdirAll(filepath.Dir(path), 0o700); err != nil {
		t.Fatal(err)
	}
	if err := os.WriteFile(path, []byte(content), 0o600); err != nil {
		t.Fatal(err)
	}
}

func recordReleaseBuild(t *testing.T, prov *artifactProvenance, phase asset.Phase, model, prompt string) {
	t.Helper()
	revision, err := sourceStateRevision(prov.root)
	if err != nil {
		t.Fatal(err)
	}
	inputs := make(map[string]string)
	if spec, ok := releasePromptSpecs[phase.Name]; ok {
		for _, relative := range append(append([]string(nil), spec.required...), spec.optional...) {
			_, digest, present, err := readReleasePromptFile(prov.root, relative)
			if err != nil {
				t.Fatal(err)
			}
			if present {
				inputs[relative] = digest
			}
		}
	}
	prov.recordBuild(phase, model, prompt, revision, inputs)
}

func rewriteFakeClaude(t *testing.T, path, script string) {
	t.Helper()
	buildNativeFakeClaude(t, path, script)
}

func quoteJSString(value string) string {
	return "'" + strings.ReplaceAll(value, "'", "\\'") + "'"
}
