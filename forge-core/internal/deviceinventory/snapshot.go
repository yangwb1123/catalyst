package deviceinventory

import (
	"crypto/sha256"
	"encoding/hex"
	"sort"
	"strings"
	"unicode"
	"unicode/utf8"
)

const (
	SnapshotCanonicalDomain = "forge.device-inventory-snapshot-canonical/v1"
	MaxSnapshotRows         = 128
	MaxSnapshotIdentifier   = 128
	MaxSnapshotOwnerBytes   = 512
)

// SnapshotOwner is an exact, caller-declared owner tuple. It is not an
// authenticated identity and is never normalized by canonicalization.
type SnapshotOwner struct {
	Issuer   string `json:"issuer"`
	Subject  string `json:"subject"`
	TenantID string `json:"tenant_id"`
}

// SnapshotRow is one caller-declared row in an owner-scoped snapshot.
type SnapshotRow struct {
	DeviceID   string        `json:"device_id"`
	InstanceID string        `json:"instance_id"`
	Owner      SnapshotOwner `json:"owner"`
}

// Snapshot is a fixed, read-only inventory declaration. ObservedAtMS is
// supplied data; canonicalization never reads a clock or verifies freshness.
type Snapshot struct {
	SnapshotID   string        `json:"snapshot_id"`
	ObservedAtMS uint64        `json:"observed_at_ms"`
	Owner        SnapshotOwner `json:"owner"`
	Rows         []SnapshotRow `json:"rows"`
}

type SnapshotError string

const (
	ErrInvalidSnapshotID     SnapshotError = "invalid_snapshot_id"
	ErrInvalidObservedAt     SnapshotError = "invalid_observed_at"
	ErrInvalidSnapshotOwner  SnapshotError = "invalid_owner"
	ErrSnapshotOwnerMismatch SnapshotError = "owner_mismatch"
	ErrDuplicateSnapshotRow  SnapshotError = "duplicate_row"
	ErrTooManySnapshotRows   SnapshotError = "too_many_rows"
)

func (e SnapshotError) Error() string { return string(e) }

// CanonicalizeSnapshot validates one caller-supplied snapshot and returns a
// copy whose rows are ordered by device ID then runner instance ID. The input
// slice is never mutated, and the result grants no inventory or execution
// authority.
func CanonicalizeSnapshot(value Snapshot) (Snapshot, error) {
	if !validSnapshotIdentifier(value.SnapshotID) {
		return Snapshot{}, ErrInvalidSnapshotID
	}
	if value.ObservedAtMS == 0 {
		return Snapshot{}, ErrInvalidObservedAt
	}
	if !validSnapshotOwner(value.Owner) {
		return Snapshot{}, ErrInvalidSnapshotOwner
	}
	if len(value.Rows) > MaxSnapshotRows {
		return Snapshot{}, ErrTooManySnapshotRows
	}
	rows := append([]SnapshotRow(nil), value.Rows...)
	for _, row := range rows {
		if !validSnapshotIdentifier(row.DeviceID) || !validSnapshotIdentifier(row.InstanceID) {
			return Snapshot{}, ErrInvalidSnapshotID
		}
		if row.Owner != value.Owner {
			return Snapshot{}, ErrSnapshotOwnerMismatch
		}
		if !validSnapshotOwner(row.Owner) {
			return Snapshot{}, ErrInvalidSnapshotOwner
		}
	}
	sort.Slice(rows, func(left, right int) bool {
		if rows[left].DeviceID != rows[right].DeviceID {
			return rows[left].DeviceID < rows[right].DeviceID
		}
		return rows[left].InstanceID < rows[right].InstanceID
	})
	for index := 1; index < len(rows); index++ {
		if rows[index-1].DeviceID == rows[index].DeviceID && rows[index-1].InstanceID == rows[index].InstanceID {
			return Snapshot{}, ErrDuplicateSnapshotRow
		}
	}
	return Snapshot{SnapshotID: value.SnapshotID, ObservedAtMS: value.ObservedAtMS, Owner: value.Owner, Rows: rows}, nil
}

// SnapshotDigest returns a domain-separated fingerprint of the canonical
// snapshot. It is an integrity label for the unverified declaration, never an
// identity proof or execution authorization.
func SnapshotDigest(value Snapshot) (string, error) {
	canonical, err := CanonicalizeSnapshot(value)
	if err != nil {
		return "", err
	}
	hasher := sha256.New()
	hasher.Write([]byte(SnapshotCanonicalDomain))
	hasher.Write([]byte{0})
	appendSnapshotField(hasher, canonical.SnapshotID)
	appendSnapshotField(hasher, formatSnapshotUint(canonical.ObservedAtMS))
	appendSnapshotOwner(hasher, canonical.Owner)
	for _, row := range canonical.Rows {
		appendSnapshotField(hasher, row.DeviceID)
		appendSnapshotField(hasher, row.InstanceID)
		appendSnapshotOwner(hasher, row.Owner)
	}
	return hex.EncodeToString(hasher.Sum(nil)), nil
}

func appendSnapshotOwner(hasher interface{ Write([]byte) (int, error) }, owner SnapshotOwner) {
	appendSnapshotField(hasher, owner.Issuer)
	appendSnapshotField(hasher, owner.Subject)
	appendSnapshotField(hasher, owner.TenantID)
}

func appendSnapshotField(hasher interface{ Write([]byte) (int, error) }, value string) {
	appendSnapshotBytes(hasher, []byte(value))
}

func appendSnapshotBytes(hasher interface{ Write([]byte) (int, error) }, value []byte) {
	length := formatSnapshotUint(uint64(len(value)))
	_, _ = hasher.Write([]byte(length))
	_, _ = hasher.Write([]byte{':'})
	_, _ = hasher.Write(value)
	_, _ = hasher.Write([]byte{0})
}

func formatSnapshotUint(value uint64) string {
	if value == 0 {
		return "0"
	}
	var buffer [20]byte
	index := len(buffer)
	for value > 0 {
		index--
		buffer[index] = byte('0' + value%10)
		value /= 10
	}
	return string(buffer[index:])
}

func validSnapshotOwner(value SnapshotOwner) bool {
	return validSnapshotOwnerPart(value.Issuer) && validSnapshotOwnerPart(value.Subject) && validSnapshotOwnerPart(value.TenantID)
}

func validSnapshotOwnerPart(value string) bool {
	return utf8.ValidString(value) && value != "" && len(value) <= MaxSnapshotOwnerBytes &&
		strings.TrimSpace(value) == value && !strings.ContainsFunc(value, unicode.IsControl)
}

func validSnapshotIdentifier(value string) bool {
	if len(value) == 0 || len(value) > MaxSnapshotIdentifier {
		return false
	}
	for index, char := range value {
		if !(char >= 'a' && char <= 'z' || char >= 'A' && char <= 'Z' || char >= '0' && char <= '9' ||
			index > 0 && (char == '.' || char == '_' || char == ':' || char == '-')) {
			return false
		}
	}
	return true
}
