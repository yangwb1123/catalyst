package appserver

import (
	"context"
	"encoding/json"
	"errors"
	"net/http"
	"net/http/httptest"
	"path/filepath"
	"strings"
	"sync"
	"sync/atomic"
	"testing"

	"forgeos/forge-core/internal/deviceidentity"
	"forgeos/forge-core/internal/deviceinventory"
	"forgeos/forge-core/internal/statefs"
)

func TestLifecycleChallengeCandidateReissuesAfterExpiryAndConsumption(t *testing.T) {
	identity, authenticator := newConversationTestIdentity(t)
	now := &atomic.Uint64{}
	now.Store(120_000)
	randomCounter := &atomic.Uint64{}
	path := filepath.Join(t.TempDir(), "lifecycle-registry.json")
	if err := statefs.EnsurePrivateDir(filepath.Dir(path)); err != nil {
		t.Fatal(err)
	}
	config := lifecycleChallengeBoundaryConfig(path, now, randomCounter)
	handler := authenticator.Handler(newLifecycleRegistryCandidateRoutes(config))
	seedLifecycleChallengeBoundary(t, handler, identity)

	initial := issueLifecycleChallengeBoundary(t, handler, identity, 60_000)
	if initial.Challenge.IssuedAtMS != 120_000 {
		t.Fatalf("initial issued_at_ms=%d", initial.Challenge.IssuedAtMS)
	}
	now.Store(initial.Challenge.ExpiresAtMS)
	expired := issueLifecycleChallengeBoundary(t, handler, identity, 60_000)
	if expired.Challenge.IssuedAtMS != initial.Challenge.ExpiresAtMS ||
		expired.Challenge.ChallengeID == initial.Challenge.ChallengeID {
		t.Fatalf("expiry reissue did not replace challenge: initial=%#v expired=%#v", initial.Challenge, expired.Challenge)
	}

	markLifecycleChallengeBoundaryConsumed(t, path, identity)
	consumed := issueLifecycleChallengeBoundary(t, handler, identity, 60_000)
	if consumed.Challenge.Consumed || consumed.Challenge.ChallengeID == expired.Challenge.ChallengeID {
		t.Fatalf("consumed challenge was not replaced: expired=%#v consumed=%#v", expired.Challenge, consumed.Challenge)
	}

	restarted := authenticator.Handler(newLifecycleRegistryCandidateRoutes(
		lifecycleChallengeBoundaryConfig(path, now, randomCounter),
	))
	active := requestConversationAPI(t, restarted, identity, http.MethodPost,
		lifecycleChallengeCandidatePath, lifecycleChallengeCandidateScope,
		"application/json", "", lifecycleChallengeCandidateBody(t, "device-a", strings.Repeat("a", 64), 60_000, 1))
	if active.Code != http.StatusConflict || !strings.Contains(active.Body.String(), `"code":"challenge_conflict"`) {
		t.Fatalf("persisted reissued challenge was not active after restart: status=%d body=%q", active.Code, active.Body.String())
	}
}

func TestLifecycleChallengeCandidateTTLAndClockBoundaries(t *testing.T) {
	current := deviceinventory.PersistedEnrollmentHeartbeatLifecycleState{
		Device: deviceidentity.DeviceBinding{ApprovalState: "approved", CredentialState: "active"},
	}
	handler := lifecycleChallengeCandidateHandler{}
	cases := []struct {
		name      string
		ttl       uint64
		valid     bool
		expiresAt uint64
	}{
		{name: "below minimum", ttl: minLifecycleChallengeTTLMS - 1},
		{name: "minimum", ttl: minLifecycleChallengeTTLMS, valid: true, expiresAt: 101_000},
		{name: "maximum", ttl: maxLifecycleChallengeTTLMS, valid: true, expiresAt: 400_000},
		{name: "above maximum", ttl: maxLifecycleChallengeTTLMS + 1},
	}
	for _, testCase := range cases {
		t.Run(testCase.name, func(t *testing.T) {
			request := httptest.NewRequest(http.MethodPost, lifecycleChallengeCandidatePath,
				strings.NewReader(lifecycleChallengeCandidateBody(t, "device-a", strings.Repeat("a", 64), testCase.ttl, 1)))
			request.Header.Set("Content-Type", "application/json")
			parsed, ok := decodeLifecycleChallengeCandidateRequest(httptest.NewRecorder(), request)
			if ok != testCase.valid {
				t.Fatalf("decode ok=%v, want %v", ok, testCase.valid)
			}
			if !ok {
				return
			}
			challenge, err := handler.issueChallenge(parsed, 100_000, current)
			if err != nil || challenge.ExpiresAtMS != testCase.expiresAt {
				t.Fatalf("challenge=%#v err=%v", challenge, err)
			}
		})
	}

	overflowRequest := lifecycleChallengeCandidateRequest{
		DeviceID: "device-a", HeartbeatSHA256: strings.Repeat("a", 64),
		TTLMS: minLifecycleChallengeTTLMS,
	}
	_, err := handler.issueChallenge(overflowRequest, ^uint64(0)-minLifecycleChallengeTTLMS+1, current)
	if !errors.Is(err, errLifecycleChallengeStale) {
		t.Fatalf("overflow issue error=%v, want stale", err)
	}
	current.ChallengeCandidate = &deviceidentity.Challenge{
		ChallengeID: "old", ChallengeSHA256: strings.Repeat("a", 64), IssuedAtMS: 100, ExpiresAtMS: 200,
	}
	if _, err := handler.issueChallenge(overflowRequest, 200, current); err != nil {
		t.Fatalf("exact expiry should reissue: %v", err)
	}
}

func TestLifecycleChallengeCandidateCASAllowsOneConcurrentIssue(t *testing.T) {
	identity, authenticator := newConversationTestIdentity(t)
	now := &atomic.Uint64{}
	now.Store(120_000)
	randomCounter := &atomic.Uint64{}
	path := filepath.Join(t.TempDir(), "lifecycle-registry.json")
	if err := statefs.EnsurePrivateDir(filepath.Dir(path)); err != nil {
		t.Fatal(err)
	}
	config := lifecycleChallengeBoundaryConfig(path, now, randomCounter)
	handler := authenticator.Handler(newLifecycleRegistryCandidateRoutes(config))
	seedLifecycleChallengeBoundary(t, handler, identity)
	body := lifecycleChallengeCandidateBody(t, "device-a", strings.Repeat("a", 64), 60_000, 1)
	statuses := runConcurrentLifecycleChallengeBoundary(t, handler, identity, body)
	var success, conflict int
	for _, status := range statuses {
		switch status {
		case http.StatusOK:
			success++
		case http.StatusConflict:
			conflict++
		default:
			t.Fatalf("unexpected concurrent challenge status=%d", status)
		}
	}
	if success != 1 || conflict != 1 {
		t.Fatalf("concurrent challenge results success=%d conflict=%d", success, conflict)
	}
	owner := deviceidentity.Owner{Issuer: identity.issuer, Subject: "account-42", TenantID: "tenant-slate"}
	adapter, err := deviceinventory.NewPersistedEnrollmentHeartbeatLifecycleFileSetWriteAdapter(path, owner)
	if err != nil {
		t.Fatal(err)
	}
	snapshot, err := adapter.ReadSnapshot()
	if err != nil {
		t.Fatal(err)
	}
	if len(snapshot.States()) != 1 || snapshot.States()[0].ChallengeCandidate == nil {
		t.Fatalf("concurrent issue did not leave one candidate: %#v", snapshot.States())
	}
}

func runConcurrentLifecycleChallengeBoundary(
	t *testing.T,
	handler http.Handler,
	identity *conversationTestIdentity,
	body string,
) []int {
	t.Helper()
	start := make(chan struct{})
	statuses := make(chan int, 2)
	var wait sync.WaitGroup
	for range 2 {
		wait.Add(1)
		go func() {
			defer wait.Done()
			<-start
			request := httptest.NewRequest(http.MethodPost, "http://127.0.0.1:7467"+lifecycleChallengeCandidatePath, strings.NewReader(body))
			request.Header.Set("Authorization", "Bearer "+identity.token(lifecycleChallengeCandidateScope))
			request.Header.Set("Content-Type", "application/json")
			response := httptest.NewRecorder()
			handler.ServeHTTP(response, request)
			statuses <- response.Result().StatusCode
		}()
	}
	close(start)
	wait.Wait()
	close(statuses)
	values := make([]int, 0, 2)
	for status := range statuses {
		values = append(values, status)
	}
	return values
}

func lifecycleChallengeBoundaryConfig(path string, now, randomCounter *atomic.Uint64) *lifecycleRegistryCandidateConfig {
	return &lifecycleRegistryCandidateConfig{
		Enabled: true, Store: newPersistedLifecycleRegistryCandidateStore(path),
		Heartbeat: &lifecycleHeartbeatCandidateConfig{
			Enabled: true, AllowUnsignedProof: true,
			Now: func(context.Context) (uint64, error) { return now.Load(), nil }, StaleAfterMS: 90_000,
		},
		Challenge: &lifecycleChallengeCandidateConfig{Enabled: true, Random: func(value []byte) error {
			seed := randomCounter.Add(1)
			for index := range value {
				value[index] = byte(seed + uint64(index))
			}
			return nil
		}},
	}
}

func seedLifecycleChallengeBoundary(t *testing.T, handler http.Handler, identity *conversationTestIdentity) {
	t.Helper()
	seed := requestConversationAPI(t, handler, identity, http.MethodPost,
		lifecycleHeartbeatCandidatePath, lifecycleHeartbeatCandidateScope,
		"application/json", "", lifecycleHeartbeatCandidateBody(t, identity.issuer, "account-42", "tenant-slate", 1, 1, 0))
	if seed.Code != http.StatusOK {
		t.Fatalf("seed heartbeat status=%d body=%q", seed.Code, seed.Body.String())
	}
}

func issueLifecycleChallengeBoundary(
	t *testing.T,
	handler http.Handler,
	identity *conversationTestIdentity,
	ttl uint64,
) lifecycleChallengeCandidateResponse {
	t.Helper()
	response := requestConversationAPI(t, handler, identity, http.MethodPost,
		lifecycleChallengeCandidatePath, lifecycleChallengeCandidateScope,
		"application/json", "", lifecycleChallengeCandidateBody(t, "device-a", strings.Repeat("a", 64), ttl, 1))
	if response.Code != http.StatusOK {
		t.Fatalf("challenge issue status=%d body=%q", response.Code, response.Body.String())
	}
	var value lifecycleChallengeCandidateResponse
	if err := json.Unmarshal(response.Body.Bytes(), &value); err != nil {
		t.Fatal(err)
	}
	return value
}

func markLifecycleChallengeBoundaryConsumed(t *testing.T, path string, identity *conversationTestIdentity) {
	t.Helper()
	owner := deviceidentity.Owner{Issuer: identity.issuer, Subject: "account-42", TenantID: "tenant-slate"}
	adapter, err := deviceinventory.NewPersistedEnrollmentHeartbeatLifecycleFileSetWriteAdapter(path, owner)
	if err != nil {
		t.Fatal(err)
	}
	snapshot, err := adapter.ReadSnapshot()
	if err != nil {
		t.Fatal(err)
	}
	states := snapshot.States()
	if len(states) != 1 || states[0].ChallengeCandidate == nil {
		t.Fatalf("challenge candidate missing before consumption: %#v", states)
	}
	consumed := *states[0].ChallengeCandidate
	consumed.Consumed = true
	states[0].ChallengeCandidate = &consumed
	if _, err := adapter.ReplaceStatesIfUnchanged(snapshot, states); err != nil {
		t.Fatalf("persist consumed challenge: %v", err)
	}
}
