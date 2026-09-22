// Package deviceinventory contains effect-free projections for future device
// inventory clients. It does not verify identity, persist heartbeats, or grant
// reservation or execution authority.
package deviceinventory

const (
	DefaultStaleAfterMS uint64 = 90_000
	MaxStaleAfterMS     uint64 = 24 * 60 * 60 * 1000

	StatusOnline   = "online"
	StatusPending  = "pending"
	StatusStale    = "stale"
	StatusOffline  = "offline"
	StatusReserved = "reserved"
	StatusCordoned = "cordoned"
	StatusRevoked  = "revoked"
)

// Observation is an already-decoded, caller-declared inventory row. The
// projection compares only the supplied fixed evaluation time.
type Observation struct {
	ApprovalState        string `json:"approval_state"`
	CordonState          string `json:"cordon_state"`
	Liveness             string `json:"liveness"`
	ReservationState     string `json:"reservation_state"`
	SnapshotObservedAtMS uint64 `json:"snapshot_observed_at_ms"`
	LeaseExpiresAtMS     uint64 `json:"lease_expires_at_ms"`
	EvaluatedAtMS        uint64 `json:"evaluated_at_ms"`
}

// Projection is a display classification. DeclaredEligible means only that
// the unverified declaration passes this pure comparison; it is never a
// scheduler decision or permission to reserve or execute.
type Projection struct {
	Status           string
	Fresh            bool
	DeclaredEligible bool
}

type ErrorCode string

const (
	ErrInvalidEvaluationTime ErrorCode = "invalid_evaluation_time"
	ErrInvalidStaleAfter     ErrorCode = "invalid_stale_after"
	ErrSnapshotFromFuture    ErrorCode = "snapshot_from_future"
	ErrLeaseBeforeSnapshot   ErrorCode = "lease_before_snapshot"
	ErrUnknownApproval       ErrorCode = "unknown_approval"
	ErrUnknownCordon         ErrorCode = "unknown_cordon"
	ErrUnknownLiveness       ErrorCode = "unknown_liveness"
	ErrUnknownReservation    ErrorCode = "unknown_reservation"
)

func (e ErrorCode) Error() string { return string(e) }

// Project classifies one declaration without reading a clock or mutating
// state. Freshness is true only when the snapshot is within the fixed bound
// and its declared lease has not expired at evaluatedAtMS.
func Project(value Observation, staleAfterMS uint64) (Projection, error) {
	if value.EvaluatedAtMS == 0 {
		return Projection{}, ErrInvalidEvaluationTime
	}
	if staleAfterMS == 0 || staleAfterMS > MaxStaleAfterMS {
		return Projection{}, ErrInvalidStaleAfter
	}
	if value.SnapshotObservedAtMS > value.EvaluatedAtMS {
		return Projection{}, ErrSnapshotFromFuture
	}
	if value.LeaseExpiresAtMS < value.SnapshotObservedAtMS {
		return Projection{}, ErrLeaseBeforeSnapshot
	}
	if !valid(value.ApprovalState, "approved", "pending", "revoked") {
		return Projection{}, ErrUnknownApproval
	}
	if !valid(value.CordonState, "clear", "cordoned") {
		return Projection{}, ErrUnknownCordon
	}
	if !valid(value.Liveness, "online", "offline") {
		return Projection{}, ErrUnknownLiveness
	}
	if !valid(value.ReservationState, "none", "reserved") {
		return Projection{}, ErrUnknownReservation
	}
	age := value.EvaluatedAtMS - value.SnapshotObservedAtMS
	fresh := age <= staleAfterMS && value.LeaseExpiresAtMS > value.EvaluatedAtMS
	status := StatusOnline
	switch {
	case value.ApprovalState == "revoked":
		status = StatusRevoked
	case value.CordonState == "cordoned":
		status = StatusCordoned
	case value.Liveness == "offline":
		status = StatusOffline
	case !fresh:
		status = StatusStale
	case value.ApprovalState == "pending":
		status = StatusPending
	case value.ReservationState == "reserved":
		status = StatusReserved
	}
	return Projection{Status: status, Fresh: fresh, DeclaredEligible: status == StatusOnline}, nil
}

func valid(value string, allowed ...string) bool {
	for _, candidate := range allowed {
		if value == candidate {
			return true
		}
	}
	return false
}
