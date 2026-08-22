package main

import (
	"os"
	"path/filepath"
	"strings"
	"testing"
)

func TestStrictReviewerRealClaudeAcceptsAndRepairsBeforeQA(t *testing.T) {
	fake := buildQAVerdictFakeClaude(t)

	t.Run("approve", func(t *testing.T) {
		root, code, output := runStrictReviewerCLI(t, fake, "L3", "approve", "explorer")
		if code != 0 {
			t.Fatalf("L3 approval exit=%d:\n%s", code, output)
		}
		assertQAVerdictPhaseCalls(t, root, map[string]int{"reviewer": 1, "qa": 1})
	})

	t.Run("request changes then approve", func(t *testing.T) {
		root, code, output := runStrictReviewerCLI(t, fake, "L4", "reject_once", "engineering")
		if code != 0 || !strings.Contains(output, "loop-back 1/3 to implementer") {
			t.Fatalf("L4 repair exit=%d:\n%s", code, output)
		}
		assertQAVerdictPhaseCalls(t, root, map[string]int{
			"implementer": 2, "reviewer": 2, "qa": 1,
		})
	})
}

func TestStrictReviewerGenericCommandRequiresExactSuccessfulPlainVerdict(t *testing.T) {
	generic := buildQAVerdictFakeGeneric(t)
	root, code, output := runStrictReviewerCLI(t, generic, "L3", "approve", "engineering")
	if code != 0 {
		t.Fatalf("generic exact approval exit=%d:\n%s", code, output)
	}
	assertQAVerdictPhaseCalls(t, root, map[string]int{"reviewer": 1, "qa": 1})
	for _, reviewer := range []string{"malformed", "wrapped_whitespace", "nonzero"} {
		t.Run(reviewer, func(t *testing.T) {
			root, code, output := runStrictReviewerCLI(t, generic, "L4", reviewer, "engineering")
			if code != 1 {
				t.Fatalf("generic %s exit=%d, want fail-closed:\n%s", reviewer, code, output)
			}
			assertQAVerdictPhaseCalls(t, root, map[string]int{"reviewer": 1, "qa": 0})
		})
	}
}

func TestStrictReviewerMalformedOrProviderFailureNeverReachesQA(t *testing.T) {
	fake := buildQAVerdictFakeClaude(t)
	for _, reviewer := range []string{
		"missing", "malformed", "trailing", "wrapped_whitespace", "qa_token", "conflicting",
		"plain_approve", "error_envelope", "multiple_envelopes", "duplicate_envelope_key",
		"case_alias_envelope", "control_prefixed",
	} {
		t.Run(reviewer, func(t *testing.T) {
			root, code, output := runStrictReviewerCLI(t, fake, "L3", reviewer, "engineering")
			if code != 1 {
				t.Fatalf("%s strict reviewer exit=%d, want fail-closed:\n%s", reviewer, code, output)
			}
			assertQAVerdictPhaseCalls(t, root, map[string]int{"reviewer": 1, "qa": 0})
		})
	}
}

func TestStrictReviewerPersistentRejectionExhaustsBeforeQA(t *testing.T) {
	fake := buildQAVerdictFakeClaude(t)
	root, code, output := runStrictReviewerCLI(t, fake, "L4", "reject_always", "engineering")
	if code != 1 || !strings.Contains(output, "could not take its required directed loop-back") {
		t.Fatalf("persistent strict rejection exit=%d:\n%s", code, output)
	}
	assertQAVerdictPhaseCalls(t, root, map[string]int{
		"reviewer": maxLoopBack + 1, "qa": 0,
	})
}

func TestLowMaterialityReviewerRemainsAdvisory(t *testing.T) {
	fake := buildQAVerdictFakeClaude(t)
	for _, level := range []string{"L0", "L2"} {
		t.Run(level, func(t *testing.T) {
			root, code, output := runStrictReviewerCLI(t, fake, level, "missing", "explorer")
			if code != 0 {
				t.Fatalf("%s advisory compatibility exit=%d:\n%s", level, code, output)
			}
			assertQAVerdictPhaseCalls(t, root, map[string]int{"reviewer": 0, "qa": 1})
		})
	}
}

func TestStrictReviewerDryExecutorFailsClosedAtReviewer(t *testing.T) {
	fake := buildQAVerdictFakeClaude(t)
	root := qaVerdictCLIFixture(t, fake)
	args := []string{
		"build", "--root", root, "--executor", "dry", "--mode", "engineering",
		"--lifecycle", "idea", "--materiality", "L3",
	}
	code, output := captureChainOutput(t, func() int { return cmdRun(args) })
	if code != 1 || !strings.Contains(output, "required agent verdict is missing or malformed") {
		t.Fatalf("strict dry run exit=%d, want reviewer fail-closed:\n%s", code, output)
	}
}

func TestStrictReviewerParallelRejectedBeforeTraceOrSpawn(t *testing.T) {
	fake := buildQAVerdictFakeClaude(t)
	root := qaVerdictCLIFixture(t, fake)
	writeQAVerdictConfig(t, root, "accepted", "approve")
	addSequentialBuildDependencies(t, root)
	args := append(shippedRunArgs("build", root, fake, false, false, "engineering"),
		"--materiality", "L3", "--parallel")
	code, output := captureChainOutput(t, func() int { return cmdRun(args) })
	if code != 1 || !strings.Contains(output, "requires serial directed loop-back") {
		t.Fatalf("strict parallel exit=%d:\n%s", code, output)
	}
	assertQAVerdictPhaseCalls(t, root, map[string]int{"planner": 0, "reviewer": 0})
	if _, err := os.Stat(filepath.Join(root, ".forge", "trace.jsonl")); !os.IsNotExist(err) {
		t.Fatalf("strict parallel rejection created trace: %v", err)
	}
}

func TestHighMaterialityMissingReviewerRejectedBeforeStateOrSpawn(t *testing.T) {
	fake := buildQAVerdictFakeClaude(t)
	root := qaVerdictCLIFixture(t, fake)
	path := filepath.Join(root, ".agent", "workflows", "build.yml")
	body, err := os.ReadFile(path)
	if err != nil {
		t.Fatal(err)
	}
	lines := strings.Split(string(body), "\n")
	filtered := lines[:0]
	for _, line := range lines {
		if !strings.Contains(line, "verdict_contract: reviewer_v2") {
			filtered = append(filtered, line)
		}
	}
	if err := os.WriteFile(path, []byte(strings.Join(filtered, "\n")), 0o600); err != nil {
		t.Fatal(err)
	}
	args := append(shippedRunArgs("build", root, fake, false, false, "engineering"), "--materiality", "L3")
	code, output := captureChainOutput(t, func() int { return cmdRun(args) })
	if code != 1 || (!strings.Contains(output, "exactly one reviewer_v2") &&
		!strings.Contains(output, "requires exactly one Build reviewer_v2")) {
		t.Fatalf("missing strict reviewer exit=%d:\n%s", code, output)
	}
	assertQAVerdictPhaseCalls(t, root, map[string]int{"planner": 0, "reviewer": 0})
	if _, err := os.Stat(filepath.Join(root, ".forge")); !os.IsNotExist(err) {
		t.Fatalf("topology rejection created runtime state: %v", err)
	}
}

func TestHighMaterialityMissingQARejectedBeforeStateOrSpawn(t *testing.T) {
	fake := buildQAVerdictFakeClaude(t)
	root := qaVerdictCLIFixture(t, fake)
	workflow := `{"id":"build","stage":"build","phases":[` +
		`{"name":"implementer","agent":"implementer"},` +
		`{"name":"reviewer","agent":"reviewer","readonly":true,"fresh_context":true,` +
		`"verdict_contract":"reviewer_v1","on_fail":{"action":"loop_back","target_phase":"implementer"}}]}`
	path := filepath.Join(root, ".agent", "workflows", "build.yml")
	if err := os.WriteFile(path, []byte(workflow), 0o600); err != nil {
		t.Fatal(err)
	}
	args := append(shippedRunArgs("build", root, fake, false, false, "engineering"), "--materiality", "L4")
	code, output := captureChainOutput(t, func() int { return cmdRun(args) })
	if code != 1 || !strings.Contains(output, "requires at least one qa_v1") {
		t.Fatalf("missing strict QA exit=%d:\n%s", code, output)
	}
	assertQAVerdictPhaseCalls(t, root, map[string]int{"implementer": 0, "reviewer": 0})
	if _, err := os.Stat(filepath.Join(root, ".forge")); !os.IsNotExist(err) {
		t.Fatalf("missing strict QA created runtime state: %v", err)
	}
}

func TestMaterialityFlagRejectsInvalidExplicitValuesBeforeStateOrSpawn(t *testing.T) {
	fake := buildQAVerdictFakeClaude(t)
	for _, flag := range []string{"--materiality=", "--materiality=l3", "--materiality=L5", "--materiality=materiality_not_bound"} {
		t.Run(flag, func(t *testing.T) {
			root := qaVerdictCLIFixture(t, fake)
			args := append(shippedRunArgs("build", root, fake, false, false, "engineering"), flag)
			code, output := captureChainOutput(t, func() int { return cmdRun(args) })
			if code != 2 || !strings.Contains(output, "--materiality") {
				t.Fatalf("invalid %s exit=%d:\n%s", flag, code, output)
			}
			assertQAVerdictPhaseCalls(t, root, map[string]int{"planner": 0})
			if _, err := os.Stat(filepath.Join(root, ".forge")); !os.IsNotExist(err) {
				t.Fatalf("invalid materiality created runtime state: %v", err)
			}
		})
	}
}

func TestRunRejectsTrailingPositionalBeforeMaterialityWithoutDowngrade(t *testing.T) {
	fake := buildQAVerdictFakeClaude(t)
	root := qaVerdictCLIFixture(t, fake)
	args := shippedRunArgs("build", root, fake, false, false, "engineering")
	args = append(args, "stray", "--materiality", "L4")
	code, output := captureChainOutput(t, func() int { return cmdRun(args) })
	if code != 2 || !strings.Contains(output, "unexpected positional") {
		t.Fatalf("trailing positional run exit=%d:\n%s", code, output)
	}
	assertQAVerdictPhaseCalls(t, root, map[string]int{"planner": 0})
	if _, err := os.Stat(filepath.Join(root, ".forge")); !os.IsNotExist(err) {
		t.Fatalf("trailing positional created runtime state: %v", err)
	}
}

func runStrictReviewerCLI(t *testing.T, fake, level, reviewer, mode string) (string, int, string) {
	t.Helper()
	root := qaVerdictCLIFixture(t, fake)
	writeQAVerdictConfig(t, root, "accepted", reviewer)
	args := append(shippedRunArgs("build", root, fake, false, false, mode), "--materiality", level)
	code, output := captureChainOutput(t, func() int { return cmdRun(args) })
	return root, code, output
}

func buildQAVerdictFakeGeneric(t *testing.T) string {
	t.Helper()
	claude := buildQAVerdictFakeClaude(t)
	generic := filepath.Join(filepath.Dir(claude), "review-command")
	if err := os.Link(claude, generic); err != nil {
		t.Fatalf("link generic reviewer command: %v", err)
	}
	return generic
}
