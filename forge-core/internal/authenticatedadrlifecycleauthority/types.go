package authenticatedadrlifecycleauthority

import (
	"crypto/sha256"
	"encoding/binary"
	"hash"
	"io/fs"
)

// Config binds protected authority material and lifecycle state outside the
// repository. StateDir and its architecture-decision-lifecycle.lock mode-0600
// leaf must be preprovisioned; authority leaves are distinct and outside it.
type Config struct {
	RepositoryRoot                      string
	AuthorityRoot                       string
	StateDir                            string
	SignatureProfilePath                string
	ApprovalTrustRootPath               string
	LifecycleTrustRootPath              string
	StateSignerSeedPath                 string
	ExtraExcludedProposalBindingSHA256s []string
}

// ExternalTrust is authenticated caller input. The authority does not infer
// root pins, epochs, or time from a request or its state file.
type ExternalTrust struct {
	PinnedApprovalTrustRootSHA256  string
	PinnedApprovalTrustEpoch       int64
	PinnedLifecycleTrustRootSHA256 string
	PinnedLifecycleTrustEpoch      int64
	ObservedAtUnixMS               int64
}

// EncodedTransitionInput carries one exact canonical ADR-0082 request.
type EncodedTransitionInput struct {
	RequestJSON []byte
}

// StoredTransition is an opaque operation result created only after strict
// durable reopen or locked exact historical replay. Its zero value is invalid.
type StoredTransition struct {
	resultJSON  []byte
	stateJSON   []byte
	sequence    int64
	disposition string
	seal        [32]byte
}

// ResultJSON returns the detached unsigned ADR-0082 delivery envelope.
func (s *StoredTransition) ResultJSON() []byte {
	if !validStoredTransition(s) {
		return nil
	}
	return cloneBytes(s.resultJSON)
}

// StateJSON returns the detached signed state image that was strictly reopened.
func (s *StoredTransition) StateJSON() []byte {
	if !validStoredTransition(s) {
		return nil
	}
	return cloneBytes(s.stateJSON)
}

// Sequence returns the lifecycle sequence identified by the result.
func (s *StoredTransition) Sequence() int64 {
	if !validStoredTransition(s) {
		return 0
	}
	return s.sequence
}

// Disposition returns either stored or exact_replay for a valid result.
func (s *StoredTransition) Disposition() string {
	if !validStoredTransition(s) {
		return ""
	}
	return s.disposition
}

type stateSnapshot struct {
	Data     []byte
	Present  bool
	identity fs.FileInfo
}

type stateSession interface {
	current() (stateSnapshot, error)
	commit(stateSnapshot, []byte) error
	readLeaf(string, int64, fs.FileMode) ([]byte, error)
	close() error
}

func newStoredTransition(result, state []byte, sequence int64,
	disposition string) (*StoredTransition, error) {
	if len(result) == 0 || len(state) == 0 || sequence < 1 ||
		(disposition != "stored" && disposition != "exact_replay") {
		return nil, coded(codeStateRejected, errInvalidStoredTransition)
	}
	value := &StoredTransition{resultJSON: cloneBytes(result), stateJSON: cloneBytes(state),
		sequence: sequence, disposition: disposition}
	value.seal = storedTransitionSeal(value)
	return value, nil
}

func validStoredTransition(value *StoredTransition) bool {
	return value != nil && len(value.resultJSON) > 0 && len(value.stateJSON) > 0 &&
		value.seal == storedTransitionSeal(value)
}

func storedTransitionSeal(value *StoredTransition) [32]byte {
	if value == nil {
		return [32]byte{}
	}
	hasher := sha256.New()
	writeSealPart(hasher, []byte("forgeos.authenticated-adr-lifecycle.stored-transition.internal.v1\x00"))
	writeSealPart(hasher, value.resultJSON)
	writeSealPart(hasher, value.stateJSON)
	writeSealInt(hasher, value.sequence)
	writeSealPart(hasher, []byte(value.disposition))
	var result [32]byte
	copy(result[:], hasher.Sum(nil))
	return result
}

func writeSealPart(hasher hash.Hash, value []byte) {
	var length [8]byte
	binary.BigEndian.PutUint64(length[:], uint64(len(value)))
	_, _ = hasher.Write(length[:])
	_, _ = hasher.Write(value)
}

func writeSealInt(hasher hash.Hash, value int64) {
	var encoded [8]byte
	binary.BigEndian.PutUint64(encoded[:], uint64(value))
	_, _ = hasher.Write(encoded[:])
}
