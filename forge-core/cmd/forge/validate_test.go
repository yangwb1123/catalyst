package main

import (
	"os"
	"path/filepath"
	"testing"
)

func TestKnownAgents_ReadsCards(t *testing.T) {
	root := t.TempDir()
	agents := filepath.Join(root, ".agent", "agents")
	if err := os.MkdirAll(agents, 0o755); err != nil {
		t.Fatal(err)
	}
	writeFileAt(t, filepath.Join(agents, "planner.md"), "planner card content")
	writeFileAt(t, filepath.Join(agents, "implementer.md"), "implementer card content")
	writeFileAt(t, filepath.Join(agents, "reviewer.md"), "reviewer card content")
	// A non-agent .md file is still treated as known (the glob is *.md, and the
	// agents dir discourages non-agent files — but if one lives there, it's read).
	writeFileAt(t, filepath.Join(agents, "README.md"), "readme")

	known := knownAgents(root)
	for _, want := range []string{"planner", "implementer", "reviewer", "README"} {
		if !known[want] {
			t.Errorf("knownAgents should include %q", want)
		}
	}
}

func TestKnownAgents_EmptyDir(t *testing.T) {
	root := t.TempDir()
	known := knownAgents(root)
	if len(known) != 0 {
		t.Errorf("knownAgents on empty dir = %v, want empty", known)
	}
}

func TestKnownAgents_MissingDir(t *testing.T) {
	root := t.TempDir()
	known := knownAgents(root)
	if known == nil {
		t.Error("knownAgents on missing dir should be empty map, not nil")
	}
}
