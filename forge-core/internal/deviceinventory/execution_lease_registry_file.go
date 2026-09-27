package deviceinventory

// This file is the small durable boundary for scheduler lease claims. It is
// intentionally separate from the lifecycle registry: heartbeat CAS owns the
// device image, while this file owns only fenced reservations and idempotent
// claim receipts. The adapter does not authenticate devices, mutate
// inventory, start a Runner, or dispatch work.

import (
	"bytes"
	"context"
	"crypto/rand"
	"encoding/hex"
	"encoding/json"
	"errors"
	"fmt"
	"io"
	"os"
	"path/filepath"
	"strings"

	"forgeos/forge-core/internal/deviceidentity"
	"forgeos/forge-core/internal/executionlease"
	"forgeos/forge-core/internal/statefs"
)

const (
	maxExecutionLeaseRegistryFileBytes = 2 << 20
	maxExecutionLeaseRegistryEntries   = 128
)

var (
	ErrExecutionLeaseRegistryInvalid = errors.New("execution lease registry file is invalid")
	ErrExecutionLeaseRegistryOwner   = errors.New("execution lease registry owner mismatch")
)

// ExecutionLeaseFencingTokenSource is injectable for deterministic contract
// tests. Production construction uses crypto/rand and never accepts a caller
// supplied fencing token.
type ExecutionLeaseFencingTokenSource func() (string, error)

// PersistedExecutionLeaseRegistryFileSetWriteAdapter owns one private,
// owner-scoped lease image. Construction is side-effect free; the parent must
// already exist and be private when Claim is called.
type PersistedExecutionLeaseRegistryFileSetWriteAdapter struct {
	path        string
	owner       deviceidentity.Owner
	tokenSource ExecutionLeaseFencingTokenSource
}

// PersistedExecutionLeaseRegistryFileSetReadAdapter reopens the same
// owner-private registry for a point-in-time proof check. It never writes the
// file and never issues a token.
type PersistedExecutionLeaseRegistryFileSetReadAdapter struct {
	path  string
	owner deviceidentity.Owner
}

// NewPersistedExecutionLeaseRegistryFileSetWriteAdapter validates fixed
// parameters without creating files or directories.
func NewPersistedExecutionLeaseRegistryFileSetWriteAdapter(
	path string,
	owner deviceidentity.Owner,
) (PersistedExecutionLeaseRegistryFileSetWriteAdapter, error) {
	return newPersistedExecutionLeaseRegistryFileSetWriteAdapter(path, owner, randomExecutionLeaseFencingToken)
}

// NewPersistedExecutionLeaseRegistryFileSetWriteAdapterWithTokenSource is
// used by contract tests to make the opaque fencing token deterministic.
func NewPersistedExecutionLeaseRegistryFileSetWriteAdapterWithTokenSource(
	path string,
	owner deviceidentity.Owner,
	source ExecutionLeaseFencingTokenSource,
) (PersistedExecutionLeaseRegistryFileSetWriteAdapter, error) {
	return newPersistedExecutionLeaseRegistryFileSetWriteAdapter(path, owner, source)
}

// NewPersistedExecutionLeaseRegistryFileSetReadAdapter validates fixed
// parameters without creating files or directories.
func NewPersistedExecutionLeaseRegistryFileSetReadAdapter(
	path string,
	owner deviceidentity.Owner,
) (PersistedExecutionLeaseRegistryFileSetReadAdapter, error) {
	if strings.TrimSpace(path) == "" || filepath.Clean(path) == "." ||
		!executionlease.RegistryOwnerValid(owner) {
		return PersistedExecutionLeaseRegistryFileSetReadAdapter{}, ErrExecutionLeaseRegistryInvalid
	}
	return PersistedExecutionLeaseRegistryFileSetReadAdapter{path: filepath.Clean(path), owner: owner}, nil
}

// Lookup returns the exact proof history entry from the private registry. A
// released entry is returned as history so callers can report an inactive
// proof without losing the epoch fence. A newer epoch is reported as stale.
func (adapter PersistedExecutionLeaseRegistryFileSetReadAdapter) Lookup(
	ctx context.Context,
	conversationID string,
	runID string,
	attemptID string,
	proof executionlease.LeaseProof,
) (executionlease.RegistryEntry, error) {
	if ctx == nil || strings.TrimSpace(adapter.path) == "" ||
		!executionlease.RegistryOwnerValid(adapter.owner) {
		return executionlease.RegistryEntry{}, ErrExecutionLeaseRegistryInvalid
	}
	if err := ctx.Err(); err != nil {
		return executionlease.RegistryEntry{}, err
	}
	lock, err := acquireLifecycleRegistryFileLock(adapter.path)
	if err != nil {
		return executionlease.RegistryEntry{}, fmt.Errorf("%w: acquire lock: %v", ErrExecutionLeaseRegistryInvalid, err)
	}
	defer func() { _ = lock.Close() }()
	if err := ctx.Err(); err != nil {
		return executionlease.RegistryEntry{}, err
	}
	_, _, _, entries, err := readExecutionLeaseRegistryFile(adapter.path, adapter.owner)
	if err != nil {
		return executionlease.RegistryEntry{}, err
	}
	highestEpoch := uint64(0)
	var matched executionlease.RegistryEntry
	foundMatch := false
	for _, entry := range entries {
		if entry.ConversationID != conversationID || entry.RunID != runID || entry.AttemptID != attemptID || entry.InstanceID != proof.TargetID {
			continue
		}
		if entry.Grant.Epoch > highestEpoch {
			highestEpoch = entry.Grant.Epoch
		}
		if entry.Grant.Proof() == proof {
			matched = entry
			foundMatch = true
		}
	}
	if highestEpoch > proof.Epoch {
		return executionlease.RegistryEntry{}, executionlease.ErrLeaseStale
	}
	if foundMatch {
		return matched, nil
	}
	return executionlease.RegistryEntry{}, executionlease.ErrLeaseNotFound
}

func newPersistedExecutionLeaseRegistryFileSetWriteAdapter(
	path string,
	owner deviceidentity.Owner,
	source ExecutionLeaseFencingTokenSource,
) (PersistedExecutionLeaseRegistryFileSetWriteAdapter, error) {
	if strings.TrimSpace(path) == "" || filepath.Clean(path) == "." ||
		!executionlease.RegistryOwnerValid(owner) || source == nil {
		return PersistedExecutionLeaseRegistryFileSetWriteAdapter{}, ErrExecutionLeaseRegistryInvalid
	}
	return PersistedExecutionLeaseRegistryFileSetWriteAdapter{
		path: filepath.Clean(path), owner: owner, tokenSource: source,
	}, nil
}

// Claim applies one scheduler claim under an inter-process file lock. An
// exact idempotency replay returns the original fenced grant and performs no
// write. The candidates must already have passed fresh inventory evaluation.
func (adapter PersistedExecutionLeaseRegistryFileSetWriteAdapter) Claim(
	ctx context.Context,
	request executionlease.ClaimRequest,
	candidates []executionlease.ClaimCandidate,
) (executionlease.RegistryEntry, bool, error) {
	if ctx == nil || strings.TrimSpace(adapter.path) == "" ||
		!executionlease.RegistryOwnerValid(adapter.owner) || adapter.tokenSource == nil {
		return executionlease.RegistryEntry{}, false, ErrExecutionLeaseRegistryInvalid
	}
	if err := ctx.Err(); err != nil {
		return executionlease.RegistryEntry{}, false, err
	}
	lock, err := acquireLifecycleRegistryFileLock(adapter.path)
	if err != nil {
		return executionlease.RegistryEntry{}, false, fmt.Errorf("%w: acquire lock: %v", ErrExecutionLeaseRegistryInvalid, err)
	}
	defer func() { _ = lock.Close() }()
	if err := ctx.Err(); err != nil {
		return executionlease.RegistryEntry{}, false, err
	}
	data, mode, present, entries, err := readExecutionLeaseRegistryFile(adapter.path, adapter.owner)
	if err != nil {
		return executionlease.RegistryEntry{}, false, err
	}
	next, entry, replayed, err := executionlease.Claim(entries, request, candidates, "")
	if errors.Is(err, executionlease.ErrTokenRequired) {
		token, tokenErr := adapter.tokenSource()
		if tokenErr != nil {
			return executionlease.RegistryEntry{}, false, fmt.Errorf("%w: fencing token source: %v", ErrExecutionLeaseRegistryInvalid, tokenErr)
		}
		next, entry, replayed, err = executionlease.Claim(entries, request, candidates, token)
	}
	if err != nil {
		return executionlease.RegistryEntry{}, false, err
	}
	if replayed {
		return entry, true, nil
	}
	encoded, err := marshalExecutionLeaseRegistryFile(adapter.owner, next)
	if err != nil {
		return executionlease.RegistryEntry{}, false, err
	}
	if err := statefs.AtomicWriteTrackedIfUnchanged(adapter.path, data, mode, present, encoded, 0o600); err != nil {
		return executionlease.RegistryEntry{}, false, fmt.Errorf("%w: commit: %v", ErrExecutionLeaseRegistryInvalid, err)
	}
	return entry, false, nil
}

// Renew applies one fenced lease renewal under the same inter-process lock as
// Claim. A replay returns the already persisted replacement grant and never
// asks the token source for another value.
func (adapter PersistedExecutionLeaseRegistryFileSetWriteAdapter) Renew(
	ctx context.Context,
	request executionlease.RenewRequest,
) (executionlease.RegistryEntry, bool, error) {
	if ctx == nil || strings.TrimSpace(adapter.path) == "" ||
		!executionlease.RegistryOwnerValid(adapter.owner) || adapter.tokenSource == nil {
		return executionlease.RegistryEntry{}, false, ErrExecutionLeaseRegistryInvalid
	}
	if err := ctx.Err(); err != nil {
		return executionlease.RegistryEntry{}, false, err
	}
	lock, err := acquireLifecycleRegistryFileLock(adapter.path)
	if err != nil {
		return executionlease.RegistryEntry{}, false, fmt.Errorf("%w: acquire lock: %v", ErrExecutionLeaseRegistryInvalid, err)
	}
	defer func() { _ = lock.Close() }()
	if err := ctx.Err(); err != nil {
		return executionlease.RegistryEntry{}, false, err
	}
	data, mode, present, entries, err := readExecutionLeaseRegistryFile(adapter.path, adapter.owner)
	if err != nil {
		return executionlease.RegistryEntry{}, false, err
	}
	next, entry, replayed, err := executionlease.Renew(entries, request, "")
	if errors.Is(err, executionlease.ErrTokenRequired) {
		token, tokenErr := adapter.tokenSource()
		if tokenErr != nil {
			return executionlease.RegistryEntry{}, false, fmt.Errorf("%w: fencing token source: %v", ErrExecutionLeaseRegistryInvalid, tokenErr)
		}
		next, entry, replayed, err = executionlease.Renew(entries, request, token)
	}
	if err != nil {
		return executionlease.RegistryEntry{}, false, err
	}
	if replayed {
		return entry, true, nil
	}
	encoded, err := marshalExecutionLeaseRegistryFile(adapter.owner, next)
	if err != nil {
		return executionlease.RegistryEntry{}, false, err
	}
	if err := statefs.AtomicWriteTrackedIfUnchanged(adapter.path, data, mode, present, encoded, 0o600); err != nil {
		return executionlease.RegistryEntry{}, false, fmt.Errorf("%w: commit: %v", ErrExecutionLeaseRegistryInvalid, err)
	}
	return entry, false, nil
}

// Release marks one active fenced lease inactive under the same inter-process
// lock as Claim and Renew. The historical epoch and token remain persisted so
// a later claim advances fencing instead of allowing a stale proof to return.
func (adapter PersistedExecutionLeaseRegistryFileSetWriteAdapter) Release(
	ctx context.Context,
	request executionlease.ReleaseRequest,
) (executionlease.RegistryEntry, bool, error) {
	if ctx == nil || strings.TrimSpace(adapter.path) == "" ||
		!executionlease.RegistryOwnerValid(adapter.owner) || adapter.tokenSource == nil {
		return executionlease.RegistryEntry{}, false, ErrExecutionLeaseRegistryInvalid
	}
	if err := ctx.Err(); err != nil {
		return executionlease.RegistryEntry{}, false, err
	}
	lock, err := acquireLifecycleRegistryFileLock(adapter.path)
	if err != nil {
		return executionlease.RegistryEntry{}, false, fmt.Errorf("%w: acquire lock: %v", ErrExecutionLeaseRegistryInvalid, err)
	}
	defer func() { _ = lock.Close() }()
	if err := ctx.Err(); err != nil {
		return executionlease.RegistryEntry{}, false, err
	}
	data, mode, present, entries, err := readExecutionLeaseRegistryFile(adapter.path, adapter.owner)
	if err != nil {
		return executionlease.RegistryEntry{}, false, err
	}
	next, entry, replayed, err := executionlease.Release(entries, request)
	if err != nil {
		return executionlease.RegistryEntry{}, false, err
	}
	if replayed {
		return entry, true, nil
	}
	encoded, err := marshalExecutionLeaseRegistryFile(adapter.owner, next)
	if err != nil {
		return executionlease.RegistryEntry{}, false, err
	}
	if err := statefs.AtomicWriteTrackedIfUnchanged(adapter.path, data, mode, present, encoded, 0o600); err != nil {
		return executionlease.RegistryEntry{}, false, fmt.Errorf("%w: commit: %v", ErrExecutionLeaseRegistryInvalid, err)
	}
	return entry, false, nil
}

type executionLeaseRegistryFileEnvelope struct {
	SchemaVersion string                         `json:"schema_version"`
	Owner         deviceidentity.Owner           `json:"owner"`
	Entries       []executionlease.RegistryEntry `json:"entries"`
}

func readExecutionLeaseRegistryFile(
	path string,
	owner deviceidentity.Owner,
) ([]byte, os.FileMode, bool, []executionlease.RegistryEntry, error) {
	directory, present, err := statefs.InspectDir(filepath.Dir(path))
	if err != nil || !present || directory.Mode().Perm()&0o077 != 0 {
		return nil, 0, false, nil, fmt.Errorf("%w: private parent required", ErrExecutionLeaseRegistryInvalid)
	}
	data, mode, present, err := statefs.ReadTracked(path, maxExecutionLeaseRegistryFileBytes)
	if err != nil {
		return nil, 0, false, nil, fmt.Errorf("%w: read: %v", ErrExecutionLeaseRegistryInvalid, err)
	}
	if !present {
		return nil, 0, false, nil, nil
	}
	if mode.Perm() != 0o600 {
		return nil, 0, false, nil, fmt.Errorf("%w: file mode must be 0600", ErrExecutionLeaseRegistryInvalid)
	}
	if err := rejectDuplicateJSONKeys(data); err != nil {
		return nil, 0, false, nil, fmt.Errorf("%w: duplicate or malformed JSON: %v", ErrExecutionLeaseRegistryInvalid, err)
	}
	decoder := json.NewDecoder(bytes.NewReader(data))
	decoder.DisallowUnknownFields()
	var envelope executionLeaseRegistryFileEnvelope
	if err := decoder.Decode(&envelope); err != nil {
		return nil, 0, false, nil, fmt.Errorf("%w: decode: %v", ErrExecutionLeaseRegistryInvalid, err)
	}
	var trailing any
	if err := decoder.Decode(&trailing); !errors.Is(err, io.EOF) {
		if err == nil {
			return nil, 0, false, nil, fmt.Errorf("%w: trailing JSON", ErrExecutionLeaseRegistryInvalid)
		}
		return nil, 0, false, nil, fmt.Errorf("%w: trailing JSON: %v", ErrExecutionLeaseRegistryInvalid, err)
	}
	return validateExecutionLeaseRegistryDecoded(data, mode, present, owner, envelope)
}

func validateExecutionLeaseRegistryDecoded(
	data []byte,
	mode os.FileMode,
	present bool,
	owner deviceidentity.Owner,
	envelope executionLeaseRegistryFileEnvelope,
) ([]byte, os.FileMode, bool, []executionlease.RegistryEntry, error) {
	if envelope.SchemaVersion != executionlease.RegistrySchemaVersion || envelope.Owner != owner ||
		len(envelope.Entries) > maxExecutionLeaseRegistryEntries {
		if envelope.Owner != owner {
			return nil, 0, false, nil, ErrExecutionLeaseRegistryOwner
		}
		return nil, 0, false, nil, ErrExecutionLeaseRegistryInvalid
	}
	for _, entry := range envelope.Entries {
		if err := entry.Validate(); err != nil {
			return nil, 0, false, nil, fmt.Errorf("%w: entry: %v", ErrExecutionLeaseRegistryInvalid, err)
		}
	}
	return append([]byte(nil), data...), mode.Perm(), present, append([]executionlease.RegistryEntry(nil), envelope.Entries...), nil
}

func marshalExecutionLeaseRegistryFile(owner deviceidentity.Owner, entries []executionlease.RegistryEntry) ([]byte, error) {
	payload, err := json.Marshal(executionLeaseRegistryFileEnvelope{
		SchemaVersion: executionlease.RegistrySchemaVersion, Owner: owner, Entries: entries,
	})
	if err != nil || len(payload)+1 > maxExecutionLeaseRegistryFileBytes {
		return nil, fmt.Errorf("%w: encode", ErrExecutionLeaseRegistryInvalid)
	}
	return append(payload, '\n'), nil
}

func randomExecutionLeaseFencingToken() (string, error) {
	bytes := make([]byte, 32)
	if _, err := rand.Read(bytes); err != nil {
		return "", err
	}
	return hex.EncodeToString(bytes), nil
}
