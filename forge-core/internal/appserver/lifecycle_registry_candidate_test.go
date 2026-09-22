package appserver

import (
	"context"
	"encoding/json"
	"net/http"
	"os"
	"path/filepath"
	"strings"
	"testing"

	"forgeos/forge-core/internal/deviceidentity"
	"forgeos/forge-core/internal/deviceinventory"
	model "forgeos/forge-core/internal/runtimebridge/model"
	"forgeos/forge-core/internal/statefs"
)

func TestLifecycleRegistryCandidateDefaultsClosed(t *testing.T) {
	identity, authenticator := newConversationTestIdentity(t)
	handler := authenticator.Handler(newLifecycleRegistryCandidateRoutes(nil))
	for _, test := range []struct {
		name, method, scope, contentType, body string
	}{
		{name: "read", method: http.MethodGet, scope: lifecycleRegistryCandidateReadScope},
		{name: "replace", method: http.MethodPut, scope: lifecycleRegistryCandidateWriteScope, contentType: "application/json", body: `{"states":[]}`},
	} {
		t.Run(test.name, func(t *testing.T) {
			response := requestConversationAPI(t, handler, identity, test.method,
				lifecycleRegistryCandidatePath, test.scope, test.contentType, "", test.body)
			if response.Code != http.StatusNotFound || response.Body.String() != string(notFoundBody) {
				t.Fatalf("disabled candidate status=%d body=%q", response.Code, response.Body.String())
			}
		})
	}
}

func TestLifecycleRegistryCandidatePersistsEmptyOwnerScopedImageAndSeparatesScopes(t *testing.T) {
	identity, authenticator := newConversationTestIdentity(t)
	root := t.TempDir()
	if err := statefs.EnsurePrivateDir(root); err != nil {
		t.Fatal(err)
	}
	path := filepath.Join(root, "lifecycle-registry.json")
	handler := authenticator.Handler(newLifecycleRegistryCandidateRoutes(&lifecycleRegistryCandidateConfig{
		Enabled: true,
		Store:   newPersistedLifecycleRegistryCandidateStore(path),
	}))

	put := requestConversationAPI(t, handler, identity, http.MethodPut,
		lifecycleRegistryCandidatePath, lifecycleRegistryCandidateWriteScope,
		"application/json", "", `{"states":[]}`)
	if put.Code != http.StatusOK {
		t.Fatalf("PUT status=%d body=%q", put.Code, put.Body.String())
	}
	var published lifecycleRegistryCandidateEnvelope
	if err := json.Unmarshal(put.Body.Bytes(), &published); err != nil {
		t.Fatalf("decode PUT response: %v", err)
	}
	owner := deviceidentity.Owner{Issuer: identity.issuer, Subject: "account-42", TenantID: "tenant-slate"}
	if published.SchemaVersion != deviceinventory.PersistedLifecycleRegistryFileSchemaVersion ||
		published.Owner != owner || published.States == nil || len(published.States) != 0 {
		t.Fatalf("published envelope=%#v", published)
	}
	if info, err := os.Stat(path); err != nil || info.Mode().Perm() != 0o600 {
		t.Fatalf("published file info=%v, want mode 0600", err)
	}

	get := requestConversationAPI(t, handler, identity, http.MethodGet,
		lifecycleRegistryCandidatePath, lifecycleRegistryCandidateReadScope, "", "", "")
	if get.Code != http.StatusOK || get.Body.String() != put.Body.String() {
		t.Fatalf("GET status=%d body=%q want PUT body=%q", get.Code, get.Body.String(), put.Body.String())
	}

	readScopeOnPut := requestConversationAPI(t, handler, identity, http.MethodPut,
		lifecycleRegistryCandidatePath, lifecycleRegistryCandidateReadScope,
		"application/json", "", `{"states":[]}`)
	if readScopeOnPut.Code != http.StatusForbidden {
		t.Fatalf("PUT with read scope status=%d body=%q", readScopeOnPut.Code, readScopeOnPut.Body.String())
	}
	writeScopeOnGet := requestConversationAPI(t, handler, identity, http.MethodGet,
		lifecycleRegistryCandidatePath, lifecycleRegistryCandidateWriteScope, "", "", "")
	if writeScopeOnGet.Code != http.StatusForbidden {
		t.Fatalf("GET with write scope status=%d body=%q", writeScopeOnGet.Code, writeScopeOnGet.Body.String())
	}
}

func TestLifecycleRegistryCandidateRejectsTransportAndOwnerBoundaryViolations(t *testing.T) {
	identity, authenticator := newConversationTestIdentity(t)
	root := t.TempDir()
	if err := statefs.EnsurePrivateDir(root); err != nil {
		t.Fatal(err)
	}
	path := filepath.Join(root, "lifecycle-registry.json")
	foreign := deviceidentity.Owner{Issuer: identity.issuer, Subject: "account-foreign", TenantID: "tenant-slate"}
	foreignAdapter, err := deviceinventory.NewPersistedEnrollmentHeartbeatLifecycleFileSetWriteAdapter(path, foreign)
	if err != nil {
		t.Fatal(err)
	}
	initial, err := foreignAdapter.ReadSnapshot()
	if err != nil {
		t.Fatal(err)
	}
	if _, err := foreignAdapter.ReplaceStatesIfUnchanged(initial, []deviceinventory.PersistedEnrollmentHeartbeatLifecycleState{}); err != nil {
		t.Fatal(err)
	}
	handler := authenticator.Handler(newLifecycleRegistryCandidateRoutes(&lifecycleRegistryCandidateConfig{
		Enabled: true,
		Store:   newPersistedLifecycleRegistryCandidateStore(path),
	}))

	foreignRead := requestConversationAPI(t, handler, identity, http.MethodGet,
		lifecycleRegistryCandidatePath, lifecycleRegistryCandidateReadScope, "", "", "")
	if foreignRead.Code != http.StatusBadGateway || !strings.Contains(foreignRead.Body.String(), `"code":"lifecycle_registry_invalid"`) {
		t.Fatalf("foreign file status=%d body=%q", foreignRead.Code, foreignRead.Body.String())
	}
	query := requestConversationAPI(t, handler, identity, http.MethodGet,
		lifecycleRegistryCandidatePath+"?limit=1", lifecycleRegistryCandidateReadScope, "", "", "")
	if query.Code != http.StatusBadRequest {
		t.Fatalf("query status=%d body=%q", query.Code, query.Body.String())
	}
	method := requestConversationAPI(t, handler, identity, http.MethodPost,
		lifecycleRegistryCandidatePath, lifecycleRegistryCandidateReadScope, "application/json", "", `{}`)
	if method.Code != http.StatusMethodNotAllowed {
		t.Fatalf("method status=%d body=%q", method.Code, method.Body.String())
	}
	missingField := requestConversationAPI(t, handler, identity, http.MethodPut,
		lifecycleRegistryCandidatePath, lifecycleRegistryCandidateWriteScope, "application/json", "", `{}`)
	if missingField.Code != http.StatusBadRequest {
		t.Fatalf("missing field status=%d body=%q", missingField.Code, missingField.Body.String())
	}
	wrongContentType := requestConversationAPI(t, handler, identity, http.MethodPut,
		lifecycleRegistryCandidatePath, lifecycleRegistryCandidateWriteScope, "text/plain", "", `{"states":[]}`)
	if wrongContentType.Code != http.StatusUnsupportedMediaType {
		t.Fatalf("content type status=%d body=%q", wrongContentType.Code, wrongContentType.Body.String())
	}
}

func TestLifecycleRegistryCandidateMapsCASConflictAndRemainsOutsideProductionConstructor(t *testing.T) {
	identity, authenticator := newConversationTestIdentity(t)
	store := &conflictingLifecycleRegistryCandidateStore{}
	handler := authenticator.Handler(newLifecycleRegistryCandidateRoutes(&lifecycleRegistryCandidateConfig{
		Enabled: true,
		Store:   store,
	}))
	response := requestConversationAPI(t, handler, identity, http.MethodPut,
		lifecycleRegistryCandidatePath, lifecycleRegistryCandidateWriteScope,
		"application/json", "", `{"states":[]}`)
	if response.Code != http.StatusConflict || store.reads != 1 || store.writes != 1 {
		t.Fatalf("conflict status=%d reads=%d writes=%d body=%q", response.Code, store.reads, store.writes, response.Body.String())
	}

	sessions := authenticator.Handler(newAuthenticatedSessionRoutes(nil, nil))
	routes, err := newRoutesWithSessions(BuildInfo{Version: "test"}, "127.0.0.1:7467", maxInFlightRequests, sessions)
	if err != nil {
		t.Fatal(err)
	}
	production := requestConversationAPI(t, routes, identity, http.MethodGet,
		lifecycleRegistryCandidatePath, lifecycleRegistryCandidateReadScope, "", "", "")
	if production.Code != http.StatusNotFound || production.Body.String() != string(notFoundBody) {
		t.Fatalf("production route status=%d body=%q", production.Code, production.Body.String())
	}
}

type conflictingLifecycleRegistryCandidateStore struct {
	reads  int
	writes int
}

func (store *conflictingLifecycleRegistryCandidateStore) ReadSnapshot(
	context.Context,
	model.Owner,
) (deviceinventory.PersistedEnrollmentHeartbeatLifecycleFileSetSnapshot, error) {
	store.reads++
	return deviceinventory.PersistedEnrollmentHeartbeatLifecycleFileSetSnapshot{}, nil
}

func (store *conflictingLifecycleRegistryCandidateStore) ReplaceStatesIfUnchanged(
	context.Context,
	model.Owner,
	deviceinventory.PersistedEnrollmentHeartbeatLifecycleFileSetSnapshot,
	[]deviceinventory.PersistedEnrollmentHeartbeatLifecycleState,
) (deviceinventory.PersistedEnrollmentHeartbeatLifecycleFileSetSnapshot, error) {
	store.writes++
	return deviceinventory.PersistedEnrollmentHeartbeatLifecycleFileSetSnapshot{}, deviceinventory.ErrPersistedLifecycleRegistryFileCASConflict
}

var _ lifecycleRegistryCandidateStore = (*conflictingLifecycleRegistryCandidateStore)(nil)
