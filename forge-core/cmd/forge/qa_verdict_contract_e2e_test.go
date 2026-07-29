package main

import (
	"os"
	"os/exec"
	"path/filepath"
	"strings"
	"testing"
)

const (
	qaVerdictE2EConfig = ".forge/qa-verdict-e2e.config"
	qaVerdictE2ECalls  = ".forge/qa-verdict-e2e.calls"
)

func TestQAVerdictContractRealCommandProcessAcceptsAndRecovers(t *testing.T) {
	fake := buildQAVerdictFakeClaude(t)

	t.Run("accepted continues", func(t *testing.T) {
		root, code, output := runQAVerdictCLI(t, fake, "accepted", "approve")
		if code != 0 {
			t.Fatalf("accepted QA exit=%d:\n%s", code, output)
		}
		assertQAVerdictPhaseCalls(t, root, map[string]int{
			"implementer": 1,
			"reviewer":    1,
			"qa":          1,
		})
	})

	t.Run("rejected loops to implementer and recovers", func(t *testing.T) {
		root, code, output := runQAVerdictCLI(t, fake, "reject_once", "approve")
		if code != 0 {
			t.Fatalf("recovering QA rejection exit=%d:\n%s", code, output)
		}
		if !strings.Contains(output, "loop-back 1/3 to implementer") {
			t.Fatalf("QA rejection did not report its directed repair loop:\n%s", output)
		}
		assertQAVerdictPhaseCalls(t, root, map[string]int{
			"planner":     1,
			"implementer": 2,
			"reviewer":    2,
			"qa":          2,
		})
	})
}

func TestQAVerdictContractMalformedRealProcessOutputFailsClosed(t *testing.T) {
	fake := buildQAVerdictFakeClaude(t)
	for _, tc := range []struct {
		name string
		qa   string
	}{
		{name: "missing", qa: "missing"},
		{name: "malformed", qa: "malformed"},
		{name: "accepted token followed by nonempty prose", qa: "trailing"},
		{name: "legacy reviewer approval cannot impersonate QA", qa: "legacy_approve"},
		{name: "plain output cannot impersonate Claude envelope", qa: "plain_accepted"},
		{name: "provider error envelope", qa: "error_envelope"},
		{name: "multiple provider envelopes", qa: "multiple_envelopes"},
		{name: "control-prefixed provider envelope", qa: "control_prefixed"},
		{name: "whitespace-wrapped token in valid envelope", qa: "wrapped_whitespace"},
	} {
		t.Run(tc.name+" fails closed", func(t *testing.T) {
			root, code, output := runQAVerdictCLI(t, fake, tc.qa, "approve")
			if code != 1 || !strings.Contains(output, "required agent verdict is missing or malformed") {
				t.Fatalf("%s QA exit=%d, want fail-closed:\n%s", tc.name, code, output)
			}
			assertQAVerdictPhaseCalls(t, root, map[string]int{"qa": 1})
		})
	}
}

func TestQAVerdictContractPersistentRejectionExhaustsLoopBudget(t *testing.T) {
	fake := buildQAVerdictFakeClaude(t)
	root, code, output := runQAVerdictCLI(t, fake, "reject_always", "approve")
	if code != 1 ||
		!strings.Contains(output, "could not take its required directed loop-back") {
		t.Fatalf("persistent QA rejection exit=%d, want exhausted fail-closed:\n%s", code, output)
	}
	assertQAVerdictPhaseCalls(t, root, map[string]int{
		"implementer": maxLoopBack + 1,
		"qa":          maxLoopBack + 1,
	})
}

func TestQAVerdictContractPreservesAdvisoryReviewerBehavior(t *testing.T) {
	fake := buildQAVerdictFakeClaude(t)

	t.Run("review request changes still loops", func(t *testing.T) {
		root, code, output := runQAVerdictCLI(t, fake, "accepted", "reject_once")
		if code != 0 {
			t.Fatalf("recovering advisory reviewer exit=%d:\n%s", code, output)
		}
		if !strings.Contains(output, "phase reviewer: reviewer verdict REQUEST_CHANGES") ||
			!strings.Contains(output, "loop-back 1/3 to implementer") {
			t.Fatalf("ordinary reviewer did not retain its directed loop:\n%s", output)
		}
		assertQAVerdictPhaseCalls(t, root, map[string]int{
			"implementer": 2,
			"reviewer":    2,
			"qa":          1,
		})
	})

	t.Run("missing advisory reviewer verdict remains fail open", func(t *testing.T) {
		root, code, output := runQAVerdictCLI(t, fake, "accepted", "missing")
		if code != 0 {
			t.Fatalf("missing advisory reviewer verdict exit=%d:\n%s", code, output)
		}
		assertQAVerdictPhaseCalls(t, root, map[string]int{
			"reviewer": 1,
			"qa":       1,
		})
	})
}

func TestQAVerdictContractDryExecutorFailsClosed(t *testing.T) {
	fake := buildQAVerdictFakeClaude(t)
	root := qaVerdictCLIFixture(t, fake)
	code, output := captureChainOutput(t, func() int {
		return cmdRun([]string{
			"build",
			"--root", root,
			"--executor", "dry",
			"--mode", "engineering",
			"--lifecycle", "idea",
		})
	})
	if code != 1 || !strings.Contains(output, "required agent verdict is missing or malformed") {
		t.Fatalf("dry executor without QA handshake exit=%d, want fail-closed:\n%s", code, output)
	}
	assertQAVerdictPhaseCalls(t, root, map[string]int{"qa": 0})
}

func TestQAVerdictContractParallelWorkflowRejectedBeforeExecution(t *testing.T) {
	fake := buildQAVerdictFakeClaude(t)
	root := qaVerdictCLIFixture(t, fake)
	writeQAVerdictConfig(t, root, "accepted", "approve")
	addSequentialBuildDependencies(t, root)

	args := shippedRunArgs("build", root, fake, false, false, "engineering")
	args = append(args, "--parallel")
	code, output := captureChainOutput(t, func() int { return cmdRun(args) })
	want := `phase qa: verdict_contract "qa_v1" requires serial directed loop-back orchestration`
	if code != 1 || !strings.Contains(output, want) {
		t.Fatalf("parallel strict-QA workflow exit=%d, want pre-execution rejection:\n%s", code, output)
	}
	assertQAVerdictPhaseCalls(t, root, map[string]int{
		"planner": 0,
		"qa":      0,
	})
}

func runQAVerdictCLI(t *testing.T, fake, qa, reviewer string) (root string, code int, output string) {
	t.Helper()
	root = qaVerdictCLIFixture(t, fake)
	writeQAVerdictConfig(t, root, qa, reviewer)
	code, output = captureChainOutput(t, func() int {
		return cmdRun(shippedRunArgs("build", root, fake, false, false, "engineering"))
	})
	return root, code, output
}

func qaVerdictCLIFixture(t *testing.T, fake string) string {
	t.Helper()
	source := repoRoot()
	if source == "" {
		t.Skip("ForgeOS repository root unavailable")
	}
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
	writeHarnessStubs(t, root)
	t.Setenv("PATH", filepath.Dir(fake)+string(os.PathListSeparator)+os.Getenv("PATH"))
	if err := os.WriteFile(filepath.Join(root, ".gitignore"), []byte(".forge/\n"), 0o600); err != nil {
		t.Fatal(err)
	}
	mustGit(t, root, "init", "-q")
	mustGit(t, root, "config", "user.name", "Forge QA E2E")
	mustGit(t, root, "config", "user.email", "forge-qa-e2e@example.invalid")
	mustGit(t, root, "add", ".")
	mustGit(t, root, "commit", "-q", "-m", "strict QA workflow fixture")
	return root
}

func writeQAVerdictConfig(t *testing.T, root, qa, reviewer string) {
	t.Helper()
	path := filepath.Join(root, filepath.FromSlash(qaVerdictE2EConfig))
	if err := os.MkdirAll(filepath.Dir(path), 0o700); err != nil {
		t.Fatal(err)
	}
	body := "qa=" + qa + "\nreviewer=" + reviewer + "\n"
	if err := os.WriteFile(path, []byte(body), 0o600); err != nil {
		t.Fatal(err)
	}
}

func addSequentialBuildDependencies(t *testing.T, root string) {
	t.Helper()
	path := filepath.Join(root, ".agent", "workflows", "build.yml")
	data, err := os.ReadFile(path)
	if err != nil {
		t.Fatal(err)
	}
	body := string(data)
	for phase, dependency := range map[string]string{
		"implementer":   "planner",
		"harness-gates": "implementer",
		"reviewer":      "harness-gates",
		"qa":            "reviewer",
	} {
		needle := "  - name: " + phase + "\n"
		replacement := needle + "    depends_on: [" + dependency + "]\n"
		if !strings.Contains(body, needle) {
			t.Fatalf("build workflow has no phase %q", phase)
		}
		body = strings.Replace(body, needle, replacement, 1)
	}
	if err := os.WriteFile(path, []byte(body), 0o600); err != nil {
		t.Fatal(err)
	}
}

func assertQAVerdictPhaseCalls(t *testing.T, root string, want map[string]int) {
	t.Helper()
	data, err := os.ReadFile(filepath.Join(root, filepath.FromSlash(qaVerdictE2ECalls)))
	if err != nil && !os.IsNotExist(err) {
		t.Fatal(err)
	}
	lines := strings.Fields(string(data))
	for phase, expected := range want {
		got := 0
		for _, line := range lines {
			if line == phase {
				got++
			}
		}
		if got != expected {
			t.Errorf("real agent phase %s calls=%d, want %d (all calls=%v)", phase, got, expected, lines)
		}
	}
}

func buildQAVerdictFakeClaude(t *testing.T) string {
	t.Helper()
	path := filepath.Join(t.TempDir(), "claude")
	sourcePath := path + ".go"
	if err := os.WriteFile(sourcePath, []byte(qaVerdictFakeClaudeSource), 0o600); err != nil {
		t.Fatal(err)
	}
	cmd := exec.Command("go", "build", "-trimpath", "-o", path, sourcePath)
	cmd.Env = append(os.Environ(), "GOCACHE="+filepath.Join(os.TempDir(), "forgeos-native-fake-gocache"))
	if output, err := cmd.CombinedOutput(); err != nil {
		t.Fatalf("build strict-QA fake Claude: %v\n%s", err, output)
	}
	return path
}

const qaVerdictFakeClaudeSource = `package main

import (
	"encoding/json"
	"io"
	"os"
	"path/filepath"
	"strings"
)

func main() {
	data, _ := io.ReadAll(os.Stdin)
	phase := phaseOf(string(data))
	call := recordCall(phase)
	config := readConfig()
	if phase == "qa" && writeSpecialQAOutput(config["qa"]) {
		return
	}
	result := resultFor(phase, call, config["qa"], config["reviewer"])
	envelope := map[string]any{
		"type": "result", "subtype": "success", "is_error": false,
		"result": result, "total_cost_usd": 0,
	}
	_ = json.NewEncoder(os.Stdout).Encode(envelope)
}

func writeSpecialQAOutput(mode string) bool {
	success := map[string]any{
		"type": "result", "subtype": "success", "is_error": false,
		"result": "QA_VERDICT: ACCEPTED",
	}
	switch mode {
	case "plain_accepted":
		_, _ = io.WriteString(os.Stdout, "QA_VERDICT: ACCEPTED")
	case "error_envelope":
		success["is_error"] = true
		_ = json.NewEncoder(os.Stdout).Encode(success)
	case "multiple_envelopes":
		_ = json.NewEncoder(os.Stdout).Encode(success)
		_ = json.NewEncoder(os.Stdout).Encode(success)
	case "control_prefixed":
		_, _ = io.WriteString(os.Stdout, "\x00")
		_ = json.NewEncoder(os.Stdout).Encode(success)
	default:
		return false
	}
	return true
}

func phaseOf(prompt string) string {
	_, rest, ok := strings.Cut(prompt, "phase=")
	if !ok {
		return ""
	}
	phase, _, _ := strings.Cut(rest, ",")
	return strings.TrimSpace(phase)
}

func readConfig() map[string]string {
	data, _ := os.ReadFile(".forge/qa-verdict-e2e.config")
	config := map[string]string{}
	for _, line := range strings.Split(string(data), "\n") {
		key, value, ok := strings.Cut(strings.TrimSpace(line), "=")
		if ok {
			config[key] = value
		}
	}
	return config
}

func recordCall(phase string) int {
	const path = ".forge/qa-verdict-e2e.calls"
	data, _ := os.ReadFile(path)
	call := 1
	for _, prior := range strings.Fields(string(data)) {
		if prior == phase {
			call++
		}
	}
	_ = os.MkdirAll(filepath.Dir(path), 0o700)
	file, err := os.OpenFile(path, os.O_CREATE|os.O_APPEND|os.O_WRONLY, 0o600)
	if err == nil {
		_, _ = file.WriteString(phase + "\n")
		_ = file.Close()
	}
	return call
}

func resultFor(phase string, call int, qa, reviewer string) string {
	switch phase {
	case "planner":
		_ = os.MkdirAll(".agent", 0o755)
		_ = os.WriteFile(".agent/CURRENT_SPRINT.md", []byte("# QA contract E2E\n"), 0o600)
		return "TASK_LIST:\n- [ ] T001: strict QA E2E — acceptance: pass — files: docs/fake.md — depends_on: none — model: sonnet — roadmap: v2"
	case "reviewer":
		switch reviewer {
		case "missing":
			return "advisory review complete"
		case "reject_once":
			if call == 1 {
				return "review finding\nVERDICT: REQUEST_CHANGES"
			}
		}
		return "VERDICT: APPROVE"
	case "qa":
		switch qa {
		case "reject_once":
			if call == 1 {
				return "QA_VERDICT: REJECTED"
			}
			return "QA_VERDICT: ACCEPTED"
		case "reject_always":
			return "QA_VERDICT: REJECTED"
		case "missing":
			return "QA completed without a machine verdict"
		case "malformed":
			return "QA_VERDICT: MAYBE"
		case "trailing":
			return "QA_VERDICT: ACCEPTED\nnonempty trailing prose"
		case "legacy_approve":
			return "VERDICT: APPROVE"
		case "wrapped_whitespace":
			return " QA_VERDICT: ACCEPTED "
		default:
			return "QA evidence complete\nQA_VERDICT: ACCEPTED\n\n"
		}
	default:
		return "phase complete"
	}
}
`
