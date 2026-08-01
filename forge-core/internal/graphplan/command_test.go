package graphplan

import (
	"bytes"
	"os"
	"path/filepath"
	"strings"
	"testing"
)

func TestCommandEmitsExactCanonicalPlanBytesFromStdin(t *testing.T) {
	var stdout, stderr bytes.Buffer
	code := Command(
		[]string{"--graph-id", "graph-fixture-v1", "--manifest-sha256", fixtureManifestSHA},
		bytes.NewReader(validSpecJSON(t)),
		&stdout,
		&stderr,
	)
	if code != 0 || stderr.Len() != 0 {
		t.Fatalf("Command code=%d stderr=%q", code, stderr.String())
	}
	if bytes.Contains(stdout.Bytes(), []byte{'\n'}) {
		t.Fatalf("stdout contains a non-canonical newline: %q", stdout.Bytes())
	}
	want, err := MarshalPlan(mustBuild(
		t, validSpec(), fixtureManifestSHA, SchedulerProtocolVersion,
	))
	if err != nil {
		t.Fatalf("MarshalPlan: %v", err)
	}
	if !bytes.Equal(stdout.Bytes(), want) {
		t.Fatalf("stdout differs from canonical plan:\n%s\nwant:\n%s", stdout.Bytes(), want)
	}
}

func TestCommandReadsExplicitBoundedFile(t *testing.T) {
	path := filepath.Join(t.TempDir(), "graph.json")
	if err := os.WriteFile(path, validSpecJSON(t), 0o600); err != nil {
		t.Fatalf("write fixture: %v", err)
	}
	var stdout, stderr bytes.Buffer
	code := Command(
		[]string{"--graph-id", "graph", "--manifest-sha256", fixtureManifestSHA, "--input", path},
		bytes.NewReader(nil),
		&stdout,
		&stderr,
	)
	if code != 0 || stdout.Len() == 0 || stderr.Len() != 0 {
		t.Fatalf("Command code=%d stdout=%q stderr=%q", code, stdout.String(), stderr.String())
	}
}

func TestCommandRejectsArgumentsAndInputWithoutDisclosure(t *testing.T) {
	secret := "TOP-SECRET-GRAPH-BODY"
	tests := []struct {
		name  string
		args  []string
		input []byte
		code  int
	}{
		{"missing binding", nil, nil, 2},
		{"positional", []string{"--graph-id", "g", "--manifest-sha256", fixtureManifestSHA, secret}, nil, 2},
		{"duplicate flag", []string{"--graph-id", "g", "--graph-id", secret, "--manifest-sha256", fixtureManifestSHA}, nil, 2},
		{"uppercase digest", []string{"--graph-id", "g", "--manifest-sha256", strings.ToUpper(fixtureManifestSHA)}, validSpecJSON(t), 1},
		{"invalid body", []string{"--graph-id", "g", "--manifest-sha256", fixtureManifestSHA}, []byte(`{"v":"TOP-SECRET-GRAPH-BODY"}`), 1},
		{"oversize", []string{"--graph-id", "g", "--manifest-sha256", fixtureManifestSHA}, bytes.Repeat([]byte(secret), MaxSpecBytes/len(secret)+2), 1},
	}
	for _, test := range tests {
		t.Run(test.name, func(t *testing.T) {
			var stdout, stderr bytes.Buffer
			code := Command(test.args, bytes.NewReader(test.input), &stdout, &stderr)
			if code != test.code {
				t.Fatalf("Command code=%d, want %d; stderr=%q", code, test.code, stderr.String())
			}
			if stdout.Len() != 0 {
				t.Fatalf("failed command wrote stdout: %q", stdout.String())
			}
			if strings.Contains(stderr.String(), secret) {
				t.Fatalf("stderr disclosed input: %q", stderr.String())
			}
		})
	}
}

func validSpec() Spec {
	return Spec{
		V: SpecVersion,
		Manager: Manager{
			AgentProfile: "integration-manager",
			Instruction:  "Coordinate frontend, backend, and SSO.",
		},
		Nodes: []Node{
			{
				NodeID: "frontend", ProjectID: "project-frontend", MemberRole: "frontend",
				AgentProfile: "implementer", Task: "Implement browser flow.",
				Acceptance: "Browser uses the shared issuer.",
			},
			{
				NodeID: "backend", ProjectID: "project-backend", MemberRole: "backend",
				AgentProfile: "implementer", Task: "Implement token verification.",
				Acceptance: "API validates the shared issuer.",
			},
			{
				NodeID: "sso", ProjectID: "project-sso", MemberRole: "sso",
				AgentProfile: "reviewer", Task: "Review relying parties.",
				Acceptance: "Both contracts agree.",
			},
		},
		Edges: []Edge{
			{FromNodeID: "frontend", ToNodeID: "sso"},
			{FromNodeID: "backend", ToNodeID: "sso"},
		},
	}
}
