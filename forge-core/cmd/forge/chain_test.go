package main

import (
	"strings"
	"testing"

	"forgeos/forge-core/internal/mode"
)

func TestChainGuardRejectsCycle(t *testing.T) {
	g := newChainGuard(5)
	if err := g.Enter("discover"); err != nil {
		t.Fatalf("first stage: %v", err)
	}
	if err := g.Enter("design"); err != nil {
		t.Fatalf("second stage: %v", err)
	}
	if err := g.Enter("discover"); err == nil || !strings.Contains(err.Error(), "cycle") {
		t.Fatalf("repeated stage error = %v, want cycle", err)
	}
}

func TestChainGuardEnforcesStageLimit(t *testing.T) {
	g := newChainGuard(2)
	if err := g.Enter("discover"); err != nil {
		t.Fatal(err)
	}
	if err := g.Enter("design"); err != nil {
		t.Fatal(err)
	}
	if err := g.Enter("review"); err == nil || !strings.Contains(err.Error(), "limit 2") {
		t.Fatalf("third stage error = %v, want limit", err)
	}
}

func TestChainPolicyAllowsHonorsCTOBuildHalt(t *testing.T) {
	cto := mode.Effective("cto", "mvp")
	if ok, reason := chainPolicyAllows(cto, "build"); ok || !strings.Contains(reason, "build=halt") {
		t.Fatalf("cto→build = (%v, %q), want blocked with reason", ok, reason)
	}
	if ok, reason := chainPolicyAllows(cto, "review"); !ok || reason != "" {
		t.Fatalf("cto→review = (%v, %q), want allowed", ok, reason)
	}
	if ok, reason := chainPolicyAllows(mode.Effective("engineering", "mvp"), "build"); !ok || reason != "" {
		t.Fatalf("engineering→build = (%v, %q), want allowed", ok, reason)
	}
	for _, stage := range []string{"deploy", "rollback"} {
		if ok, reason := chainPolicyAllows(cto, stage); ok || !strings.Contains(reason, "action stages") {
			t.Errorf("cto→%s = (%v, %q), want blocked action stage", stage, ok, reason)
		}
	}
}
