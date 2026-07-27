package orchestrator

import (
	"context"
	"errors"
	"strings"
	"testing"

	"forgeos/forge-core/internal/asset"
)

func TestCommandExecutorChildEnvIsLeastPrivilegeByDefault(t *testing.T) {
	t.Setenv("GITHUB_TOKEN", "must-not-pass")
	t.Setenv("AWS_ACCESS_KEY_ID", "must-not-pass")
	t.Setenv("SSH_AUTH_SOCK", "/must/not/pass")
	t.Setenv("FORGE_TEST_CONTEXT", "explicit-only")
	t.Setenv("FORGE_API_KEY", "must-not-pass")
	t.Setenv("LC_PRIVATE_TOKEN", "must-not-pass")
	t.Setenv("LC_ALL", "C")
	t.Setenv("CUSTOM_AGENT_TOKEN", "explicit-only")

	got := envNames(CommandExecutor{}.childEnv(0))
	for _, denied := range []string{
		"GITHUB_TOKEN", "AWS_ACCESS_KEY_ID", "SSH_AUTH_SOCK",
		"CUSTOM_AGENT_TOKEN", "FORGE_TEST_CONTEXT", "FORGE_API_KEY",
		"LC_PRIVATE_TOKEN",
	} {
		if got[denied] {
			t.Errorf("sensitive parent variable %s leaked without an explicit grant", denied)
		}
	}
	for _, allowed := range []string{"PATH", "LC_ALL", agentDepthEnv} {
		if !got[allowed] {
			t.Errorf("minimum child environment lacks %s", allowed)
		}
	}
}

func TestCommandExecutorChildEnvAllowsExactExplicitGrant(t *testing.T) {
	t.Setenv("CUSTOM_AGENT_TOKEN", "explicit")
	t.Setenv("FORGE_TEST_CONTEXT", "explicit")
	t.Setenv("FORGE_OTHER_CONTEXT", "must-not-pass")
	got := envNames(CommandExecutor{
		EnvAllow: []string{"CUSTOM_AGENT_TOKEN", "FORGE_TEST_CONTEXT"},
	}.childEnv(1))
	if !got["CUSTOM_AGENT_TOKEN"] {
		t.Fatal("an explicitly granted variable must reach the child")
	}
	if !got["FORGE_TEST_CONTEXT"] {
		t.Fatal("an explicitly granted FORGE_ variable must reach the child")
	}
	if got["FORGE_OTHER_CONTEXT"] {
		t.Fatal("granting one FORGE_ variable must not grant its prefix siblings")
	}
}

func TestCommandExecutorRestrictedChildEnvRemovesAmbientDiscovery(t *testing.T) {
	t.Setenv("PATH", "/attacker/bin")
	t.Setenv("HOME", "/attacker/home")
	t.Setenv("SHELL", "/attacker/shell")
	t.Setenv("TMPDIR", "/attacker/tmp")
	t.Setenv("XDG_CONFIG_HOME", "/attacker/xdg")
	t.Setenv("CLAUDE_CONFIG_DIR", "/attacker/claude")
	t.Setenv("ANTHROPIC_API_KEY", "authorized")
	t.Setenv("LC_ALL", "C")
	env := CommandExecutor{
		RestrictedEnv: true,
		EnvAllow:      []string{"ANTHROPIC_API_KEY"},
	}.childEnv(0)
	got := envValues(env)
	for _, denied := range []string{
		"HOME", "SHELL", "TMPDIR", "XDG_CONFIG_HOME", "CLAUDE_CONFIG_DIR",
	} {
		if _, ok := got[denied]; ok {
			t.Errorf("restricted child inherited ambient discovery variable %s", denied)
		}
	}
	if got["PATH"] != restrictedAgentPath || got["ANTHROPIC_API_KEY"] != "authorized" ||
		got["LC_ALL"] != "C" {
		t.Fatalf("restricted child environment = %v", got)
	}
}

func TestCommandExecutorInvalidEnvGrantFailsBeforeBuild(t *testing.T) {
	built := false
	ex := CommandExecutor{
		Build: func(asset.Phase, string) []string {
			built = true
			return []string{"true"}
		},
		EnvAllow: []string{"AWS_*"},
	}
	err := ex.Execute(context.Background(), asset.Phase{Name: "implementer"}, "balanced")
	var execErr *ExecError
	if !errors.As(err, &execErr) || execErr.Kind != KindConfig {
		t.Fatalf("invalid environment grant = %T %v, want KindConfig", err, err)
	}
	if built {
		t.Fatal("invalid environment grant must fail before command construction")
	}
}

func TestCommandExecutorInputUsesStdinNotArgv(t *testing.T) {
	const prompt = "repository context that must not appear in argv"
	var log string
	ex := CommandExecutor{
		Build:          func(asset.Phase, string) []string { return []string{"sh", "-c", "cat", "-p", prompt} },
		PromptViaStdin: true,
		Log:            func(line string) { log = line },
	}
	if err := ex.Execute(context.Background(), asset.Phase{Name: "planner"}, "balanced"); err != nil {
		t.Fatalf("stdin execution: %v", err)
	}
	arrow := strings.Index(log, " -> ")
	if arrow < 0 || !strings.Contains(log[arrow:], prompt) {
		t.Fatalf("child did not receive stdin prompt: %q", log)
	}
	if strings.Contains(log[:arrow], prompt) {
		t.Fatalf("prompt leaked into logged argv: %q", log[:arrow])
	}
}

func envNames(env []string) map[string]bool {
	out := make(map[string]bool, len(env))
	for _, kv := range env {
		name, _, ok := strings.Cut(kv, "=")
		if ok {
			out[name] = true
		}
	}
	return out
}

func envValues(env []string) map[string]string {
	out := make(map[string]string, len(env))
	for _, kv := range env {
		if name, value, ok := strings.Cut(kv, "="); ok {
			out[name] = value
		}
	}
	return out
}
