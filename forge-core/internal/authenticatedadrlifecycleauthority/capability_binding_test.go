//go:build unix && !aix && !solaris

package authenticatedadrlifecycleauthority

import (
	"encoding/base64"
	"io/fs"
	"testing"

	approvalauthority "forgeos/forge-core/internal/authenticatedadrapprovalauthority"
)

type observedStateSession struct {
	stateSession
	seedPath   string
	mutatePath string
	seedReads  int
	commits    int
}

func (s *observedStateSession) readLeaf(path string, maximum int64,
	mode fs.FileMode) ([]byte, error) {
	if path == s.seedPath {
		s.seedReads++
	}
	value, err := s.stateSession.readLeaf(path, maximum, mode)
	if err == nil && path == s.mutatePath && len(value) > 0 {
		value[len(value)-1] ^= 1
	}
	return value, err
}

func (s *observedStateSession) commit(expected stateSnapshot, next []byte) error {
	s.commits++
	return s.stateSession.commit(expected, next)
}

func TestFreshTransitionBindsEveryOpaqueAuthorizationInput(t *testing.T) {
	t.Run("prerequisite fields", testEveryPrerequisiteFieldBinding)
	t.Run("proposal bytes", func(t *testing.T) {
		fixture := newAuthorityFixture(t)
		authorization := fixture.approvalStored(t)
		request := loadRawObject(t, fixture.lifecycleInput(t, authorization).RequestJSON)
		request["proposal_document_base64url"] = base64.RawURLEncoding.EncodeToString(
			append(cloneBytes(fixture.proposal), '!'))
		resealLifecycleRequest(t, request, fixture.lifecycleRequestPrivate)
		assertRejectedBeforeSeed(t, fixture, authorization,
			EncodedTransitionInput{RequestJSON: canonicalForTest(t, request)})
	})
	t.Run("different stored authorization", testCrossAuthorizationBinding)
}

func testEveryPrerequisiteFieldBinding(t *testing.T) {
	fixture := newAuthorityFixture(t)
	authorization := fixture.approvalStored(t)
	original := fixture.lifecycleInput(t, authorization)
	base := loadRawObject(t, original.RequestJSON)["acceptance_prerequisite"].(map[string]any)
	for field := range base {
		t.Run(field, func(t *testing.T) {
			request := loadRawObject(t, original.RequestJSON)
			prerequisite := request["acceptance_prerequisite"].(map[string]any)
			mutatePrerequisiteField(prerequisite, field)
			if field != "prerequisite_sha256" {
				resealPrerequisite(t, prerequisite)
			}
			resealLifecycleRequest(t, request, fixture.lifecycleRequestPrivate)
			assertRejectedBeforeSeed(t, fixture, authorization,
				EncodedTransitionInput{RequestJSON: canonicalForTest(t, request)})
		})
	}
}

func mutatePrerequisiteField(prerequisite map[string]any, field string) {
	switch value := prerequisite[field].(type) {
	case string:
		prerequisite[field] = value + "x"
	case int64:
		prerequisite[field] = value + 1
	case map[string]any:
		mutated := cloneObject(value)
		mutated["mutation_marker"] = true
		prerequisite[field] = mutated
	default:
		panic("unsupported prerequisite field type")
	}
}

func testCrossAuthorizationBinding(t *testing.T) {
	fixture := newAuthorityFixture(t)
	fixture.retargetProposal(t, "ADR-9301", "ADR-9301-capability-a.md", nil)
	firstAuthorization := fixture.approvalStoredIn(t, "approval-capability-a")
	firstInput := fixture.lifecycleInput(t, firstAuthorization)
	fixture.retargetProposal(t, "ADR-9302", "ADR-9302-capability-b.md", nil)
	secondAuthorization := fixture.approvalStoredIn(t, "approval-capability-b")
	assertRejectedBeforeSeed(t, fixture, secondAuthorization, firstInput)
}

func resealPrerequisite(t *testing.T, prerequisite map[string]any) {
	t.Helper()
	prerequisite["prerequisite_sha256"] = ""
	digest, err := digestFor("prerequisite", prerequisite)
	if err != nil {
		t.Fatal(err)
	}
	prerequisite["prerequisite_sha256"] = digest
}

func assertRejectedBeforeSeed(t *testing.T, fixture *authorityFixture,
	authorization *approvalauthority.StoredAuthorization, input EncodedTransitionInput) {
	t.Helper()
	deps := productionDependencies
	var observed *observedStateSession
	deps.openState = func(config Config) (stateSession, error) {
		base, err := openProtectedState(config)
		if err != nil {
			return nil, err
		}
		observed = &observedStateSession{stateSession: base, seedPath: config.StateSignerSeedPath}
		return observed, nil
	}
	stored, err := transitionAndStoreWith(fixture.lifecycleConfig, input,
		authorization, fixture.lifecycleTrust(), deps)
	if stored != nil || observed == nil || observed.seedReads != 0 || observed.commits != 0 {
		t.Fatalf("binding failure leaked output/seed/commit: %v %+v", stored, observed)
	}
	assertLifecycleCode(t, err, codeAuthorizationRejected)
}
