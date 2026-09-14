package runtimebridge

import (
	"context"
	"crypto/sha256"
	"encoding/json"
	"fmt"
	intentmodel "forgeos/forge-core/internal/runtimebridge/intentmodel"
	model "forgeos/forge-core/internal/runtimebridge/model"
	"os"
	"os/exec"
	"path/filepath"
	"testing"
	"time"
)

func TestPendingRunIntentToRustRPCWhenConfigured(t *testing.T) {
	fixture := newPendingIntentIntegrationFixture(t)
	defer fixture.cancel()
	first := submitPendingIntentFixture(t, fixture)
	assertPendingIntentInitialViews(t, fixture, first)
	assertPendingIntentAfterRevocation(t, fixture, first)
}

type pendingIntentIntegrationFixture struct {
	ctx            context.Context
	cancel         context.CancelFunc
	client         *Client
	owner          model.Owner
	conversationID string
	profile        intentmodel.ServerExecutionProfile
	grantID        string
	prompt         string
	idempotencyKey string
}

func newPendingIntentIntegrationFixture(t *testing.T) pendingIntentIntegrationFixture {
	t.Helper()
	ctx, cancel, executable, runtimeState, projectID := preparePendingIntentProject(t)
	appState := filepath.Join(t.TempDir(), "app-server-state")
	makeDirectory(t, appState)
	client, err := New(Config{
		Executable: executable, AppServerStateDir: appState,
		RuntimeStateDir: runtimeState, Timeout: 5 * time.Second,
	})
	if err != nil {
		cancel()
		t.Fatal(err)
	}
	owner := model.Owner{
		Issuer: "https://identity.example", Subject: "pending-intent-integration-account",
		TenantID: "pending-intent-integration-tenant",
	}
	conversation, err := client.CreateOwnedConversation(ctx, owner,
		model.ConversationScope{Kind: "project", ID: projectID}, "pending intent integration", pendingIntentIntegrationKey(t))
	if err != nil {
		cancel()
		t.Fatalf("create owner-bound Project Conversation: %v", err)
	}
	profile := intentmodel.ServerExecutionProfile{ID: "profile-integration-1", SHA256: sha256.Sum256([]byte("fixed test profile"))}
	expiresAtMS := uint64(time.Now().Add(time.Hour).UnixMilli())
	grantID := grantPendingIntentConsentForIntegration(t, ctx, client, owner, projectID, profile, expiresAtMS)
	return pendingIntentIntegrationFixture{
		ctx: ctx, cancel: cancel, client: client, owner: owner, conversationID: conversation.ID,
		profile: profile, grantID: grantID, prompt: "compute this bounded task\nwith a second line",
		idempotencyKey: pendingIntentIntegrationKey(t),
	}
}

func preparePendingIntentProject(
	t *testing.T,
) (context.Context, context.CancelFunc, string, string, string) {
	t.Helper()
	executable := os.Getenv("FORGE_RUNTIME_BIN")
	if executable == "" {
		t.Skip("set FORGE_RUNTIME_BIN for Go-to-Rust pending intent integration")
	}
	ctx, cancel := context.WithTimeout(context.Background(), 30*time.Second)
	runtimeState := filepath.Join(t.TempDir(), "runtime-state")
	makeDirectory(t, runtimeState)
	initializePendingIntentHubForIntegration(t, ctx, executable, runtimeState)
	projectID := seedPendingIntentProject(t, ctx, executable, runtimeState)
	return ctx, cancel, executable, runtimeState, projectID
}

func seedPendingIntentProject(t *testing.T, ctx context.Context, executable, runtimeState string) string {
	t.Helper()
	projectPath := t.TempDir()
	seed := exec.CommandContext(ctx, executable, "--state-dir", runtimeState, "--json", "-C", projectPath,
		"session", "new", "--title", "pending intent project seed")
	seed.Env = []string{}
	seedOutput, err := seed.CombinedOutput()
	if err != nil {
		t.Fatalf("seed Project scope: %v: %s", err, seedOutput)
	}
	var response struct {
		Session struct {
			Scope struct {
				ID string `json:"id"`
			} `json:"scope"`
		} `json:"session"`
	}
	if err := json.Unmarshal(seedOutput, &response); err != nil || response.Session.Scope.ID == "" {
		t.Fatalf("seeded Project ID=%q decode=%v output=%q", response.Session.Scope.ID, err, seedOutput)
	}
	return response.Session.Scope.ID
}

func submitPendingIntentFixture(t *testing.T, fixture pendingIntentIntegrationFixture) intentmodel.PendingRunIntentSubmissionResult {
	t.Helper()
	result, err := fixture.client.SubmitOwnedPromptRunIntent(fixture.ctx, fixture.owner, fixture.conversationID,
		fixture.prompt, fixture.idempotencyKey, 1, fixture.profile)
	if err != nil || result.Replayed || result.Intent.Status != "pending" || result.Intent.ProfileID != fixture.profile.ID ||
		result.Intent.AggregateVersion != 2 || result.InitialEvent.Type != "submitted" {
		t.Fatalf("first pending intent=%#v error=%v", result, err)
	}
	return result
}

func assertPendingIntentInitialViews(
	t *testing.T,
	fixture pendingIntentIntegrationFixture,
	first intentmodel.PendingRunIntentSubmissionResult,
) {
	t.Helper()
	page, err := fixture.client.OwnedConversationPendingRunIntents(fixture.ctx, fixture.owner, fixture.conversationID, nil, 25)
	if err != nil || len(page.Intents) != 1 || page.Intents[0] != first.Intent || page.HasMore {
		t.Fatalf("pending intent page=%#v error=%v", page, err)
	}
	timeline, err := fixture.client.OwnedConversationPendingRunIntentTimeline(fixture.ctx, fixture.owner,
		fixture.conversationID, first.Intent.IntentID, 0, 128)
	if err != nil || len(timeline.Events) != 1 || timeline.Events[0] != first.InitialEvent || timeline.HasMore {
		t.Fatalf("pending intent timeline=%#v error=%v", timeline, err)
	}
	runs, err := fixture.client.OwnedConversationRuns(fixture.ctx, fixture.owner, fixture.conversationID, nil, 25)
	if err != nil || len(runs.Runs) != 0 {
		t.Fatalf("pending intent unexpectedly created a Run: %#v error=%v", runs, err)
	}
}

func assertPendingIntentAfterRevocation(
	t *testing.T,
	fixture pendingIntentIntegrationFixture,
	first intentmodel.PendingRunIntentSubmissionResult,
) {
	t.Helper()
	if _, _, replayed, err := fixture.client.AppendOwnedPrompt(fixture.ctx, fixture.owner, fixture.conversationID,
		"later Prompt", pendingIntentIntegrationKey(t), first.Intent.AggregateVersion); err != nil || replayed {
		t.Fatalf("advance Conversation after pending intent: replayed=%v error=%v", replayed, err)
	}
	revokePendingIntentConsentForIntegration(t, fixture.ctx, fixture.client, fixture.owner, fixture.grantID)
	changedProfile := intentmodel.ServerExecutionProfile{ID: "profile-integration-2", SHA256: sha256.Sum256([]byte("changed test profile"))}
	replay, err := fixture.client.SubmitOwnedPromptRunIntent(fixture.ctx, fixture.owner, fixture.conversationID,
		fixture.prompt, fixture.idempotencyKey, 0, changedProfile)
	if err != nil || !replay.Replayed || replay.Intent != first.Intent || replay.Prompt != first.Prompt || replay.InitialEvent != first.InitialEvent {
		t.Fatalf("changed-policy retry did not return original receipt: %#v error=%v", replay, err)
	}
	assertPendingIntentConflictsAndProjection(t, fixture, first, changedProfile)
}

func assertPendingIntentConflictsAndProjection(
	t *testing.T,
	fixture pendingIntentIntegrationFixture,
	first intentmodel.PendingRunIntentSubmissionResult,
	changedProfile intentmodel.ServerExecutionProfile,
) {
	t.Helper()
	if _, err := fixture.client.SubmitOwnedPromptRunIntent(fixture.ctx, fixture.owner, fixture.conversationID,
		"changed body", fixture.idempotencyKey, 3, changedProfile); err == nil || err.(*Error).Code != "conflict" {
		t.Fatalf("same key with changed Prompt did not conflict: %v", err)
	}
	if _, err := fixture.client.SubmitOwnedPromptRunIntent(fixture.ctx, fixture.owner, fixture.conversationID,
		"fresh body", pendingIntentIntegrationKey(t), 3, changedProfile); err == nil || err.(*Error).Code != "conflict" {
		t.Fatalf("revoked consent allowed a fresh intent: %v", err)
	}
	foreign := fixture.owner
	foreign.TenantID = "another-tenant"
	if _, err := fixture.client.OwnedConversationPendingRunIntents(fixture.ctx, foreign, fixture.conversationID, nil, 25); err == nil || err.(*Error).Code != "not_found" {
		t.Fatalf("foreign owner could read pending intents: %v", err)
	}
	prompts, err := fixture.client.OwnedConversationPrompts(fixture.ctx, fixture.owner, fixture.conversationID, nil, 8)
	if err != nil || len(prompts.Prompts) != 2 || prompts.Prompts[1].Content != fixture.prompt {
		t.Fatalf("stored Prompt history=%#v error=%v", prompts, err)
	}
	page, err := fixture.client.OwnedConversationPendingRunIntents(fixture.ctx, fixture.owner, fixture.conversationID, nil, 25)
	if err != nil || len(page.Intents) != 1 || page.Intents[0].IntentID != first.Intent.IntentID {
		t.Fatalf("replay or rejected requests created another intent: %#v error=%v", page, err)
	}
}

func initializePendingIntentHubForIntegration(t *testing.T, ctx context.Context, executable, runtimeState string) {
	t.Helper()
	command := exec.CommandContext(ctx, executable, "--state-dir", runtimeState, "session", "list")
	command.Env = []string{}
	output, err := command.CombinedOutput()
	if err != nil {
		t.Fatalf("initialize Runtime Hub: %v: %s", err, output)
	}
}

func grantPendingIntentConsentForIntegration(
	t *testing.T,
	ctx context.Context,
	client *Client,
	owner model.Owner,
	projectID string,
	profile intentmodel.ServerExecutionProfile,
	expiresAtMS uint64,
) string {
	t.Helper()
	result, err := client.GrantProjectExecutionConsent(ctx, owner, projectID,
		profile.ID, profile.SHA256, expiresAtMS, pendingIntentIntegrationKey(t))
	if err != nil || result.Replayed || result.Grant.GrantID == "" ||
		result.Grant.ProjectID != projectID || result.Grant.ProfileID != profile.ID ||
		result.Grant.ProfileSHA256 != profile.SHA256 || result.Grant.ExpiresAtMS != expiresAtMS {
		t.Fatalf("grant model.Project consent: result=%#v error=%v", result, err)
	}
	return result.Grant.GrantID
}

func revokePendingIntentConsentForIntegration(
	t *testing.T,
	ctx context.Context,
	client *Client,
	owner model.Owner,
	grantID string,
) {
	t.Helper()
	result, err := client.RevokeProjectExecutionConsent(ctx, owner, grantID, pendingIntentIntegrationKey(t))
	if err != nil || result.Replayed || result.Revocation.GrantID != grantID || result.Revocation.EventID == "" {
		t.Fatalf("revoke model.Project consent: result=%#v error=%v", result, err)
	}
}

func pendingIntentIntegrationKey(t *testing.T) string {
	t.Helper()
	value, err := newRequestID()
	if err != nil {
		t.Fatal(fmt.Errorf("generate integration idempotency key: %w", err))
	}
	return "pending-" + value
}
