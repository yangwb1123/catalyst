package outputbindingstore

import (
	"bytes"
	"encoding/json"
	"fmt"
	"path/filepath"

	"forgeos/forge-core/internal/outputbinding"
	"forgeos/forge-core/internal/statefs"
)

const (
	claimLedgerName   = "agent-output-preflight-claims.jsonl"
	claimAnchorName   = "agent-output-preflight-claims.head.json"
	claimAnchorFormat = "forgeos.agent-output-preflight-claim-head.v1"
)

type claimAnchor struct {
	Format        string `json:"_format"`
	Count         int    `json:"count"`
	JournalSHA256 string `json:"journal_sha256"`
}

type claimKey struct {
	runID, workflow, phase string
}

// ClaimPath returns the private journal containing every pre-spawn binding
// claimed by the runtime, including attempts that never produced a receipt.
func ClaimPath(root string) string {
	return filepath.Join(root, ".forge", claimLedgerName)
}

func claimAnchorPath(root string) string {
	return filepath.Join(root, ".forge", claimAnchorName)
}

// ClaimPreflight durably reserves one exact attempt/challenge/binding before
// the child process is spawned. Claims are never deleted: an interrupted
// attempt must remain unavailable after restart.
func (store *Store) ClaimPreflight(binding outputbinding.PreflightBinding) error {
	if store == nil || store.root == "" {
		return fmt.Errorf("output binding store: repository root is required")
	}
	store.mu.Lock()
	defer store.mu.Unlock()
	claims, data, err := store.loadAnchoredClaims()
	if err != nil {
		return err
	}
	if err := validateClaimAppend(claims, binding); err != nil {
		return err
	}
	encoded, err := outputbinding.CanonicalPreflightJSON(binding)
	if err != nil {
		return fmt.Errorf("output binding store: encode preflight claim: %w", err)
	}
	line := append(encoded, '\n')
	snapshot := ledgerSnapshot{bytes: int64(len(data))}
	if err := validateAppendCapacity(snapshot, int64(len(line)), store.limits()); err != nil {
		return fmt.Errorf("output binding store: preflight claim: %w", err)
	}
	if len(claims) >= store.limits().entries {
		return fmt.Errorf("output binding store: preflight claim limit %d reached", store.limits().entries)
	}
	if err := statefs.EnsurePrivateDir(filepath.Join(store.root, ".forge")); err != nil {
		return err
	}
	next := append(bytes.Clone(data), line...)
	// Publish the anchor first. A crash before the journal replacement leaves
	// an ahead-of-journal anchor and therefore fails closed; publishing the
	// journal first would leave a rollback window with no monotonic witness.
	if err := writeClaimAnchor(store.root, next, len(claims)+1); err != nil {
		return err
	}
	if err := commitLedger(ClaimPath(store.root), data, line); err != nil {
		return fmt.Errorf("output binding store: commit preflight claim: %w", err)
	}
	return nil
}

// LoadPreflightClaims verifies the complete detached claim journal.
func (store *Store) LoadPreflightClaims() ([]outputbinding.PreflightBinding, error) {
	if store == nil || store.root == "" {
		return nil, fmt.Errorf("output binding store: repository root is required")
	}
	store.mu.Lock()
	defer store.mu.Unlock()
	claims, _, err := store.loadAnchoredClaims()
	return claims, err
}

func (store *Store) loadAnchoredClaims() ([]outputbinding.PreflightBinding, []byte, error) {
	claims, data, err := loadClaims(ClaimPath(store.root), store.limits())
	if err != nil {
		return nil, nil, err
	}
	anchor, present, err := readClaimAnchor(store.root)
	if err != nil {
		return nil, nil, err
	}
	if !present {
		if len(claims) > 0 {
			return nil, nil, fmt.Errorf("output binding store: preflight claim anchor is missing")
		}
		return claims, data, nil
	}
	if anchor.Count > len(claims) {
		return nil, nil, fmt.Errorf("output binding store: preflight claim journal rolled back below anchored head")
	}
	prefix := claimPrefix(data, anchor.Count)
	if outputbinding.SHA256(prefix) != anchor.JournalSHA256 {
		return nil, nil, fmt.Errorf("output binding store: preflight claim journal differs from anchored prefix")
	}
	if anchor.Count < len(claims) {
		return nil, nil, fmt.Errorf("output binding store: preflight claim anchor lags the journal")
	}
	return claims, data, nil
}

// RequireReceiptClaim proves that an accepted receipt's exact reconstructed
// preflight was consumed by the pre-spawn journal. It does not by itself make
// the receipt current or authorized for resume.
func (store *Store) RequireReceiptClaim(receipt outputbinding.AgentOutputReceipt) error {
	if err := outputbinding.ValidateReceipt(receipt); err != nil {
		return err
	}
	claims, err := store.LoadPreflightClaims()
	if err != nil {
		return err
	}
	for _, claim := range claims {
		if claim.BindingSHA256 == receipt.BindingSHA256 && claim.Challenge == receipt.Challenge &&
			claim.RunID == receipt.RunID && claim.Workflow == receipt.Workflow &&
			claim.Phase == receipt.Phase && claim.Attempt == receipt.Attempt {
			return nil
		}
	}
	return fmt.Errorf("output binding store: receipt lacks its exact pre-spawn claim")
}

// RequirePreflightClaim proves that a finalized command still has its exact
// anchored pre-spawn claim immediately before accepted receipt publication.
func (store *Store) RequirePreflightClaim(want outputbinding.PreflightBinding) error {
	if err := outputbinding.ValidatePreflight(want); err != nil {
		return err
	}
	claims, err := store.LoadPreflightClaims()
	if err != nil {
		return err
	}
	for _, claim := range claims {
		if claim == want {
			return nil
		}
	}
	return fmt.Errorf("output binding store: exact preflight claim is absent")
}

func loadClaims(path string, limits ledgerLimits) ([]outputbinding.PreflightBinding, []byte, error) {
	data, present, err := statefs.ReadRegularUnmodified(path, limits.bytes)
	if err != nil {
		return nil, nil, fmt.Errorf("output binding store: read preflight claims: %w", err)
	}
	if !present || len(data) == 0 {
		return []outputbinding.PreflightBinding{}, []byte{}, nil
	}
	if data[len(data)-1] != '\n' || bytes.Count(data, []byte{'\n'}) > limits.entries {
		return nil, nil, fmt.Errorf("output binding store: preflight claim journal is truncated or oversized")
	}
	lines := bytes.Split(data[:len(data)-1], []byte{'\n'})
	claims := make([]outputbinding.PreflightBinding, len(lines))
	for index, line := range lines {
		claim, decodeErr := outputbinding.DecodeCanonicalPreflight(line)
		if decodeErr != nil {
			return nil, nil, fmt.Errorf("output binding store: preflight claim line %d: %w", index+1, decodeErr)
		}
		claims[index] = claim
	}
	if err := validateClaims(claims); err != nil {
		return nil, nil, fmt.Errorf("output binding store: invalid preflight claims: %w", err)
	}
	return claims, bytes.Clone(data), nil
}

func validateClaimAppend(existing []outputbinding.PreflightBinding,
	candidate outputbinding.PreflightBinding) error {
	claims := make([]outputbinding.PreflightBinding, len(existing)+1)
	copy(claims, existing)
	claims[len(existing)] = candidate
	if err := validateClaims(claims); err != nil {
		return fmt.Errorf("output binding store: candidate preflight claim: %w", err)
	}
	return nil
}

func validateClaims(claims []outputbinding.PreflightBinding) error {
	attempts := map[claimKey]int64{}
	challenges, bindings := map[string]bool{}, map[string]bool{}
	for index, claim := range claims {
		if err := outputbinding.ValidatePreflight(claim); err != nil {
			return fmt.Errorf("item %d: %w", index, err)
		}
		key := claimKey{claim.RunID, claim.Workflow, claim.Phase}
		if claim.Attempt <= attempts[key] || challenges[claim.Challenge] || bindings[claim.BindingSHA256] {
			return fmt.Errorf("item %d reuses or does not advance attempt/challenge/binding", index)
		}
		attempts[key] = claim.Attempt
		challenges[claim.Challenge], bindings[claim.BindingSHA256] = true, true
	}
	return nil
}

func writeClaimAnchor(root string, journal []byte, count int) error {
	anchor := claimAnchor{
		Format: claimAnchorFormat, Count: count,
		JournalSHA256: outputbinding.SHA256(journal),
	}
	data, err := json.Marshal(anchor)
	if err != nil {
		return fmt.Errorf("output binding store: encode preflight claim anchor: %w", err)
	}
	if err := statefs.AtomicWrite(claimAnchorPath(root), data, 0o600); err != nil {
		return fmt.Errorf("output binding store: commit preflight claim anchor: %w", err)
	}
	return nil
}

func readClaimAnchor(root string) (claimAnchor, bool, error) {
	data, present, err := statefs.ReadRegularUnmodified(claimAnchorPath(root), 1<<10)
	if err != nil || !present {
		return claimAnchor{}, present, err
	}
	var anchor claimAnchor
	decoder := json.NewDecoder(bytes.NewReader(data))
	decoder.DisallowUnknownFields()
	if err := decoder.Decode(&anchor); err != nil {
		return claimAnchor{}, true, fmt.Errorf("output binding store: decode preflight claim anchor: %w", err)
	}
	var trailing any
	if err := decoder.Decode(&trailing); err == nil {
		return claimAnchor{}, true, fmt.Errorf("output binding store: preflight claim anchor has trailing JSON")
	}
	canonical, err := json.Marshal(anchor)
	if err != nil || !bytes.Equal(canonical, data) || anchor.Format != claimAnchorFormat ||
		anchor.Count < 1 || anchor.Count > defaultMaxReceipts ||
		!validClaimDigest(anchor.JournalSHA256) {
		return claimAnchor{}, true, fmt.Errorf("output binding store: invalid preflight claim anchor")
	}
	return anchor, true, nil
}

func validClaimDigest(value string) bool {
	if len(value) != 64 {
		return false
	}
	for _, char := range value {
		if !(char >= '0' && char <= '9' || char >= 'a' && char <= 'f') {
			return false
		}
	}
	return true
}

func claimPrefix(data []byte, count int) []byte {
	position := 0
	for position < len(data) && count > 0 {
		next := bytes.IndexByte(data[position:], '\n')
		if next < 0 {
			return nil
		}
		position += next + 1
		count--
	}
	if count != 0 {
		return nil
	}
	return data[:position]
}
