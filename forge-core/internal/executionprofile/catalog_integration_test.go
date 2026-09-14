package executionprofile

import (
	"context"
	"crypto/sha256"
	"encoding/json"
	intentmodel "forgeos/forge-core/internal/runtimebridge/intentmodel"
	model "forgeos/forge-core/internal/runtimebridge/model"
	"os"
	"os/exec"
	"path/filepath"
	"testing"
	"time"

	"forgeos/forge-core/internal/runtimebridge"
)

func TestProfileResolutionUsesOwnerFilteredRustHubContextWhenConfigured(t *testing.T) {
	executable := os.Getenv("FORGE_RUNTIME_BIN")
	if executable == "" {
		t.Skip("set FORGE_RUNTIME_BIN for profile catalog to Rust Hub integration")
	}
	ctx, cancel := context.WithTimeout(context.Background(), 30*time.Second)
	defer cancel()
	runtimeState := filepath.Join(t.TempDir(), "runtime-state")
	if err := os.Mkdir(runtimeState, 0o700); err != nil {
		t.Fatal(err)
	}
	initializeProfileHub(t, ctx, executable, runtimeState)
	projectPath := t.TempDir()
	seeded := seedProfileProjectConversation(t, ctx, executable, runtimeState, projectPath)
	appState := filepath.Join(t.TempDir(), "app-server-state")
	if err := os.Mkdir(appState, 0o700); err != nil {
		t.Fatal(err)
	}
	client, err := runtimebridge.New(runtimebridge.Config{
		Executable: executable, AppServerStateDir: appState, RuntimeStateDir: runtimeState,
	})
	if err != nil {
		t.Fatal(err)
	}
	owner := model.Owner{Issuer: "https://identity.example", Subject: "profile-owner", TenantID: "tenant-profile"}
	conversation, err := client.CreateOwnedConversation(ctx, owner, seeded.Scope,
		"profile-bound conversation", "profile-create-1")
	if err != nil {
		t.Fatalf("create owner-bound Project Conversation: %v", err)
	}
	profile := intentmodel.ServerExecutionProfile{
		ID: "profile-build-v1", SHA256: sha256.Sum256([]byte("canonical profile manifest v1")),
	}
	catalog, err := New([]Binding{{ProjectID: seeded.Scope.ID, Profile: profile}})
	if err != nil {
		t.Fatal(err)
	}
	resolved, err := catalog.ResolveConversationProfile(ctx, client, owner, conversation.ID)
	if err != nil || resolved != profile {
		t.Fatalf("resolved profile=%#v error=%v", resolved, err)
	}
	assertProfileCatalogOwnerAndScopeBoundaries(t, ctx, client, owner, catalog, conversation.ID)
}

func seedProfileProjectConversation(t *testing.T, ctx context.Context, executable, runtimeState, projectPath string) model.Conversation {
	t.Helper()
	seed := exec.CommandContext(ctx, executable, "--state-dir", runtimeState, "--json", "-C", projectPath,
		"session", "new", "--title", "profile catalog Project seed")
	seed.Env = []string{}
	seedOutput, err := seed.CombinedOutput()
	if err != nil {
		t.Fatalf("seed Project scope: %v: %s", err, seedOutput)
	}
	var seeded struct {
		Session model.Conversation `json:"session"`
	}
	if err := json.Unmarshal(seedOutput, &seeded); err != nil ||
		seeded.Session.Scope.Kind != "project" || seeded.Session.Scope.ID == "" {
		t.Fatalf("seeded Project Conversation=%#v decode=%v output=%q", seeded, err, seedOutput)
	}
	return seeded.Session
}

func assertProfileCatalogOwnerAndScopeBoundaries(
	t *testing.T,
	ctx context.Context,
	client *runtimebridge.Client,
	owner model.Owner,
	catalog *Catalog,
	conversationID string,
) {
	t.Helper()
	foreign := owner
	foreign.TenantID = "another-tenant"
	if _, err := catalog.ResolveConversationProfile(ctx, client, foreign, conversationID); err == nil || err.(*runtimebridge.Error).Code != "not_found" {
		t.Fatalf("foreign owner resolution error=%v", err)
	}
	global, err := client.CreateOwnedConversation(ctx, owner, model.ConversationScope{Kind: "global"},
		"global conversation", "profile-create-2")
	if err != nil {
		t.Fatalf("create Global Conversation: %v", err)
	}
	if _, err := catalog.ResolveConversationProfile(ctx, client, owner, global.ID); err == nil || err.(*runtimebridge.Error).Code != "conflict" {
		t.Fatalf("Global Conversation profile resolution error=%v", err)
	}
}

func initializeProfileHub(t *testing.T, ctx context.Context, executable, runtimeState string) {
	t.Helper()
	command := exec.CommandContext(ctx, executable, "--state-dir", runtimeState, "session", "list")
	command.Env = []string{}
	output, err := command.CombinedOutput()
	if err != nil {
		t.Fatalf("initialize Runtime Hub: %v: %s", err, output)
	}
}
