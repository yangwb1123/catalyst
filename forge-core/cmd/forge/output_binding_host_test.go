package main

import (
	"strings"
	"testing"

	"forgeos/forge-core/internal/asset"
)

func TestBoundCommandRequiresRealCrossProcessRunLock(t *testing.T) {
	wf := asset.Workflow{Stage: "build", OutputBindingContract: asset.OutputBindingContractLocalDigestV1}
	if err := validateOutputBindingHostSupport(wf, runOpts{executor: "command"}, false); err == nil ||
		!strings.Contains(err.Error(), "cross-process") {
		t.Fatalf("unsupported host error = %v", err)
	}
	if err := validateOutputBindingHostSupport(wf, runOpts{executor: "dry"}, false); err != nil {
		t.Fatalf("dry run unexpectedly required a process lock: %v", err)
	}
	if err := validateOutputBindingHostSupport(asset.Workflow{Stage: "build"}, runOpts{executor: "command"}, false); err != nil {
		t.Fatalf("legacy workflow compatibility changed: %v", err)
	}
}

func TestBoundEvolveAlsoRequiresRealCrossProcessRunLock(t *testing.T) {
	wf := asset.Workflow{Stage: "evolve", OutputBindingContract: asset.OutputBindingContractLocalDigestV1}
	if err := validateOutputBindingHostSupport(wf, runOpts{executor: "command"}, false); err == nil {
		t.Fatal("bound Evolve command execution accepted a host without process locking")
	}
}
