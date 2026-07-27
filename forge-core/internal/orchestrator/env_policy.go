package orchestrator

import (
	"fmt"
	"os"
	"strings"
)

// defaultAgentEnv is the minimum host context needed by ordinary CLI tools.
// Credential families (AWS_*, GITHUB_*, SSH_*, etc.) are deliberately absent.
var defaultAgentEnv = map[string]struct{}{
	"COLORTERM":           {},
	"HOME":                {},
	"LANG":                {},
	"LANGUAGE":            {},
	"LC_ALL":              {},
	"LC_CTYPE":            {},
	"LC_MESSAGES":         {},
	"LOGNAME":             {},
	"NODE_EXTRA_CA_CERTS": {},
	"NO_COLOR":            {},
	"PATH":                {},
	"SHELL":               {},
	"SSL_CERT_DIR":        {},
	"SSL_CERT_FILE":       {},
	"TEMP":                {},
	"TERM":                {},
	"TMP":                 {},
	"TMPDIR":              {},
	"USER":                {},
	"XDG_CACHE_HOME":      {},
	"XDG_CONFIG_HOME":     {},
	"XDG_DATA_HOME":       {},
}

var restrictedAgentEnv = map[string]struct{}{
	"LANG":                {},
	"LANGUAGE":            {},
	"LC_ALL":              {},
	"LC_CTYPE":            {},
	"LC_MESSAGES":         {},
	"NODE_EXTRA_CA_CERTS": {},
	"SSL_CERT_DIR":        {},
	"SSL_CERT_FILE":       {},
}

const restrictedAgentPath = "/usr/bin:/bin"

// environmentConfigError rejects malformed allow-list entries before Build or
// process creation. Exact names only are supported: wildcard credential grants
// are too broad for a least-privilege boundary.
func (c CommandExecutor) environmentConfigError(phase string) error {
	for _, name := range c.EnvAllow {
		if !validEnvName(name) {
			return configErr(phase, fmt.Errorf("invalid --agent-env name %q", name))
		}
	}
	return nil
}

func validEnvName(name string) bool {
	if name == "" || (name[0] != '_' && !asciiLetter(name[0])) {
		return false
	}
	for i := 1; i < len(name); i++ {
		if name[i] != '_' && !asciiLetter(name[i]) && (name[i] < '0' || name[i] > '9') {
			return false
		}
	}
	return true
}

func asciiLetter(b byte) bool {
	return b >= 'A' && b <= 'Z' || b >= 'a' && b <= 'z'
}

// childEnv filters the parent environment and injects FORGE_AGENT_DEPTH once.
// POSIX leaves duplicate-key resolution unspecified, so every inherited key is
// collapsed by name and the trusted depth counter is appended last.
func (c CommandExecutor) childEnv(depth int) []string {
	base := defaultAgentEnv
	if c.RestrictedEnv {
		base = restrictedAgentEnv
	}
	allowed := make(map[string]struct{}, len(base)+len(c.EnvAllow))
	for name := range base {
		allowed[name] = struct{}{}
	}
	for _, name := range c.EnvAllow {
		allowed[name] = struct{}{}
	}
	out := make([]string, 0, len(allowed)+1)
	seen := make(map[string]struct{}, len(allowed))
	for _, kv := range os.Environ() {
		name, _, ok := strings.Cut(kv, "=")
		if !ok || name == agentDepthEnv || !allowedAgentEnv(name, allowed) {
			continue
		}
		if _, duplicate := seen[name]; duplicate {
			continue
		}
		seen[name] = struct{}{}
		out = append(out, kv)
	}
	if c.RestrictedEnv {
		out = append(out, "PATH="+restrictedAgentPath)
	}
	return append(out, fmt.Sprintf("%s=%d", agentDepthEnv, depth+1))
}

func allowedAgentEnv(name string, exact map[string]struct{}) bool {
	_, ok := exact[name]
	return ok
}
