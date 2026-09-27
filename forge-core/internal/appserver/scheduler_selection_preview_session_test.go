//go:build linux && !android

package appserver

import (
	"context"
	"os"
	"os/exec"
	"path/filepath"
	"testing"
	"time"

	"forgeos/forge-core/internal/deviceidentity"
	"forgeos/forge-core/internal/runtimebridge"
	runtimebridgemodel "forgeos/forge-core/internal/runtimebridge/model"
)

// seedSchedulerSelectionPreviewSession creates the durable Conversation and
// completed Run that the real Sessions Gate must discover before it renders a
// scheduler-selection candidate. The scheduler route itself remains a pure
// preview and does not consume this Run or create an Attempt.
func seedSchedulerSelectionPreviewSession(
	t *testing.T,
	executable, runtimeState string,
	owner deviceidentity.Owner,
) (string, string) {
	t.Helper()
	projectPath, projectID, _ := seedRuntimeConversationScopes(t, executable, runtimeState)
	if err := os.WriteFile(
		filepath.Join(projectPath, "README.md"),
		[]byte("deterministic scheduler selection preview fixture\n"),
		0o600,
	); err != nil {
		t.Fatal(err)
	}
	bridgeStateDir := t.TempDir()
	bridge, err := runtimebridge.New(runtimebridge.Config{
		Executable: executable, AppServerStateDir: bridgeStateDir,
		RuntimeStateDir: runtimeState, Timeout: 5 * time.Second,
	})
	if err != nil {
		t.Fatal(err)
	}
	bridgeOwner := runtimebridgemodel.Owner{
		Issuer: owner.Issuer, Subject: owner.Subject, TenantID: owner.TenantID,
	}
	conversation, err := bridge.CreateOwnedConversation(
		context.Background(), bridgeOwner,
		runtimebridgemodel.ConversationScope{Kind: "project", ID: projectID},
		"Scheduler selection preview E2E", "scheduler-selection-preview-e2e-create",
	)
	if err != nil {
		t.Fatalf("create Runtime conversation for scheduler selection Gate E2E: %v", err)
	}
	prompt, _, replayed, err := bridge.AppendOwnedPrompt(
		context.Background(), bridgeOwner, conversation.ID,
		"evaluate a scheduler selection preview", "scheduler-selection-preview-e2e-prompt", 1,
	)
	if err != nil || replayed || prompt.ID == "" {
		t.Fatalf("append Runtime Prompt for scheduler selection Gate E2E: prompt=%#v replayed=%v err=%v", prompt, replayed, err)
	}
	ctx, cancel := context.WithTimeout(context.Background(), 30*time.Second)
	defer cancel()
	command := exec.CommandContext(
		ctx, executable, "--state-dir", runtimeState, "--json",
		"-C", projectPath, "run", "start", conversation.ID, prompt.ID, "--read", "README.md",
	)
	command.Env = []string{}
	if output, err := command.CombinedOutput(); err != nil || len(output) == 0 {
		t.Fatalf("seed scheduler selection preview Run: %v: %s", err, output)
	}
	page, err := bridge.OwnedConversationRuns(
		context.Background(), bridgeOwner, conversation.ID, nil, 25,
	)
	if err != nil || len(page.Runs) != 1 || page.Runs[0].PromptID != prompt.ID {
		t.Fatalf("read scheduler selection preview Run page=%#v err=%v", page, err)
	}
	return conversation.ID, page.Runs[0].RunID
}
