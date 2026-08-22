package authenticatedadrapprovalauthority

import (
	"errors"
	"fmt"
	"io/fs"
	"os"
	"path/filepath"
	"strings"
	"testing"

	contract "forgeos/forge-core/internal/authenticatedadrapprovalcontract"
)

type memoryState struct {
	root      []byte
	seed      []byte
	snapshot  stateSnapshot
	commitErr error
	closeErr  error
	corrupt   bool
	commits   int
	rootReads int
	seedReads int
}

func (s *memoryState) current() (stateSnapshot, error) {
	return stateSnapshot{Present: s.snapshot.Present, Data: cloneBytes(s.snapshot.Data)}, nil
}

func (s *memoryState) commit(expected stateSnapshot, next []byte) error {
	s.commits++
	if s.commitErr != nil {
		return s.commitErr
	}
	if !snapshotsEqual(expected, s.snapshot) {
		return errStateConflict
	}
	s.snapshot = stateSnapshot{Present: true, Data: cloneBytes(next)}
	if s.corrupt && len(s.snapshot.Data) > 0 {
		s.snapshot.Data[len(s.snapshot.Data)-1] ^= 1
	}
	return nil
}

func (s *memoryState) readLeaf(relative string, _ int64, _ fs.FileMode) ([]byte, error) {
	switch relative {
	case "root.json":
		s.rootReads++
		return cloneBytes(s.root), nil
	case "state.seed":
		s.seedReads++
		return cloneBytes(s.seed), nil
	default:
		return nil, fmt.Errorf("unknown memory leaf")
	}
}

func (s *memoryState) close() error { return s.closeErr }

func TestAuthorizeReturnsNoOutputAcrossPersistenceFailures(t *testing.T) {
	cases := []struct {
		name   string
		mutate func(*memoryState)
		code   Code
	}{
		{"pre-publication", func(state *memoryState) {
			state.commitErr = errors.New("write failed")
		}, codePersistenceFailed},
		{"ambiguous publication", func(state *memoryState) {
			state.commitErr = errStateUncertain
		}, codePersistenceUncertain},
		{"strict reopen mismatch", func(state *memoryState) {
			state.corrupt = true
		}, codePersistenceUncertain},
		{"close after publish", func(state *memoryState) {
			state.closeErr = errors.New("unlock failed")
		}, codePersistenceUncertain},
	}
	for _, test := range cases {
		t.Run(test.name, func(t *testing.T) {
			fixture := newServiceFixture(t)
			state := fixtureMemoryState(t, fixture)
			test.mutate(state)
			result, err := authorizeAndStoreWith(memoryConfig(), fixture.encoded(t),
				fixture.trust(), memoryDependencies(state))
			if result != nil {
				t.Fatal("persistence failure returned a bundle")
			}
			assertCode(t, err, test.code)
		})
	}
}

func TestPostRenameUncertaintyCannotReturnStoredAuthorization(t *testing.T) {
	fixture := newServiceFixture(t)
	config := fixture.config(t)
	deps := productionDependencies
	deps.openState = func(config Config) (stateSession, error) {
		return openProtectedStateWith(config, faultCommitPort{fail: "directory-sync"})
	}
	stored, err := authorizeAndStoreWith(config, fixture.encoded(t), fixture.trust(), deps)
	if stored != nil {
		t.Fatal("post-rename uncertainty returned a stored authorization")
	}
	assertCode(t, err, codePersistenceUncertain)
	if _, statErr := os.Lstat(filepath.Join(config.AuthorityRoot, config.StateDir,
		stateLedgerFile)); statErr != nil {
		t.Fatalf("fault was not injected after publication: %v", statErr)
	}
}

func TestProspectiveOutputFailuresOccurBeforeSeedOrCommit(t *testing.T) {
	fixture := newServiceFixture(t)
	t.Run("shape ceiling", func(t *testing.T) {
		state := fixtureMemoryState(t, fixture)
		deps := memoryDependencies(state)
		deps.preflightOutput = func(*contract.AuthorizationInput, *contract.ReceiptDraft,
			*contract.Ledger, int64) error {
			return coded(codeCapacityExhausted, errors.New("prospective bundle exceeds ceiling"))
		}
		result, err := authorizeAndStoreWith(memoryConfig(), fixture.encoded(t),
			fixture.trust(), deps)
		if result != nil || state.seedReads != 0 || state.commits != 0 {
			t.Fatalf("preflight failure leaked output/seed/commit: %v %d %d",
				result, state.seedReads, state.commits)
		}
		assertCode(t, err, codeCapacityExhausted)
	})
	t.Run("exact signed bundle", func(t *testing.T) {
		state := fixtureMemoryState(t, fixture)
		deps := memoryDependencies(state)
		deps.validateOutput = func(*contract.AuthorizationInput, *contract.Receipt,
			*contract.Ledger, ExternalTrust) ([]byte, error) {
			return nil, coded(codeCapacityExhausted, errors.New("signed bundle exceeds ceiling"))
		}
		result, err := authorizeAndStoreWith(memoryConfig(), fixture.encoded(t),
			fixture.trust(), deps)
		if result != nil || state.seedReads != 1 || state.commits != 0 {
			t.Fatalf("signed precommit failure state: %v %d %d",
				result, state.seedReads, state.commits)
		}
		assertCode(t, err, codeCapacityExhausted)
	})
}

func TestCallerOwnedInputsAreSnapshottedBeforePreflight(t *testing.T) {
	fixture := newServiceFixture(t)
	state := fixtureMemoryState(t, fixture)
	deps := memoryDependencies(state)
	reached, proceed := make(chan struct{}), make(chan struct{})
	readRoot := deps.readTrustRoot
	deps.readTrustRoot = func(config Config) ([]byte, error) {
		close(reached)
		<-proceed
		return readRoot(config)
	}
	encoded := fixture.encoded(t)
	config := memoryConfig()
	config.ExtraExcludedProposalBindingSHA256s = []string{strings.Repeat("a", 64)}
	type outcome struct {
		result *StoredAuthorization
		err    error
	}
	done := make(chan outcome, 1)
	go func() {
		result, err := authorizeAndStoreWith(config, encoded, fixture.trust(), deps)
		done <- outcome{result: result, err: err}
	}()
	<-reached
	encoded.Request[0] ^= 1
	config.ExtraExcludedProposalBindingSHA256s[0] =
		fixture.policy["proposal_binding"].(map[string]any)["proposal_binding_sha256"].(string)
	close(proceed)
	completed := <-done
	if completed.err != nil || completed.result == nil {
		t.Fatalf("caller mutation affected snapshotted request: %v", completed.err)
	}
}

func fixtureMemoryState(t *testing.T, fixture *serviceFixture) *memoryState {
	t.Helper()
	seed := fixture.private[fixture.byUsage["approval_authorization_state_sign"]].Seed()
	return &memoryState{root: testCanonical(t, fixture.root), seed: cloneBytes(seed)}
}

func memoryConfig() Config {
	return Config{RepositoryRoot: "/test/repository", AuthorityRoot: "/test/authority",
		StateDir: "state", TrustRootPath: "root.json", StateSignerSeedPath: "state.seed"}
}

func memoryDependencies(state stateSession) dependencies {
	memory := state.(*memoryState)
	return dependencies{checkPlatform: func() error { return nil },
		readTrustRoot:   func(Config) ([]byte, error) { return cloneBytes(memory.root), nil },
		openState:       func(Config) (stateSession, error) { return state, nil },
		preflightOutput: preflightStoredOutputShape,
		validateOutput:  validateProspectiveOutput}
}

var _ stateSession = (*memoryState)(nil)
