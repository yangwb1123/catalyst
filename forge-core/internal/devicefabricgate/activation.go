// Package devicefabricgate contains the fail-closed activation policy for the
// future device-aware execution fabric.  It is deliberately a pure value
// package: evaluating a request reads no files, opens no listener, and does
// not change an ADR lifecycle state.
package devicefabricgate

import "sort"

// SchemaVersion identifies the machine-readable activation result contract.
const SchemaVersion = "forge.device-fabric-activation-gate/v1"

// Mode is the only progression understood by the activation gate.  The order
// is intentionally not inferred from a caller-provided boolean: every mode
// has an explicit policy/evidence boundary.
type Mode string

const (
	ModeOff       Mode = "off"
	ModeInventory Mode = "inventory"
	ModeObserve   Mode = "observe"
	ModeExecute   Mode = "execute"
	ModeMigrate   Mode = "migrate"
	ModeFederate  Mode = "federate"
)

// Decision records the lifecycle fields that must be supplied by the
// deployment owner.  Proposed ADR v2 documents intentionally do not satisfy
// Accepted: both the id and timestamp are required in addition to status.
type Decision struct {
	Status           string `json:"status"`
	AcceptanceID     string `json:"acceptance_id"`
	AcceptedAtUnixMS int64  `json:"accepted_at_unix_ms"`
	PlanningOnly     bool   `json:"planning_only"`
}

func (decision Decision) accepted() bool {
	return decision.Status == "accepted" && decision.AcceptanceID != "" &&
		decision.AcceptedAtUnixMS > 0 && !decision.PlanningOnly
}

// Evidence is the reviewable evidence inventory for each activation step.
// These flags are declarations consumed by the gate; the gate does not treat
// them as proof and does not manufacture missing evidence.
type Evidence struct {
	CoordinatorOwnerIsolation    bool `json:"coordinator_owner_isolation"`
	DeviceIdentityProof          bool `json:"device_identity_proof"`
	OwnerApprovalAndRevocation   bool `json:"owner_approval_and_revocation"`
	HeartbeatCASAndFreshness     bool `json:"heartbeat_cas_and_freshness"`
	InventoryOwnerScope          bool `json:"inventory_owner_scope"`
	DisabledDefaultAndRouteClose bool `json:"disabled_default_and_route_close"`
	SecurityReview               bool `json:"security_review"`

	RunnerIsolation              bool `json:"runner_isolation"`
	LeaseFencing                 bool `json:"lease_fencing"`
	CancellationAndUncertainWork bool `json:"cancellation_and_uncertain_work"`
	VaultArtifactAuthorization   bool `json:"vault_artifact_authorization"`
	AuditOutbox                  bool `json:"audit_outbox"`
}

// Request is an explicit activation attempt.  ADR-0039 is the policy root;
// ADR-0113 and ADR-0114 are required for the authenticated owner and device
// inventory boundaries.  Execution has a separate decision slot by design.
type Request struct {
	Mode    Mode
	ADR0039 Decision
	ADR0113 Decision
	ADR0114 Decision
	P4      Decision

	Evidence Evidence
}

// Result is deterministic and safe to expose in diagnostics.  Reasons are
// stable machine-readable codes sorted lexicographically; no credential,
// device identifier, or request payload is copied into the result.
type Result struct {
	SchemaVersion string
	Mode          Mode
	Allowed       bool
	Reasons       []string
}

// Evaluate applies the staged activation policy.  ModeOff is always allowed
// and is the default for a zero-value Request.  All other modes fail closed
// until every required decision and evidence item is present.  Migrate and
// Federate remain blocked because they require decisions that are not part of
// this package.
func Evaluate(request Request) Result {
	mode := request.Mode
	if mode == "" {
		mode = ModeOff
	}
	result := Result{SchemaVersion: SchemaVersion, Mode: mode}
	if mode == ModeOff {
		result.Allowed = true
		return result
	}

	switch mode {
	case ModeInventory, ModeObserve, ModeExecute, ModeMigrate, ModeFederate:
	default:
		return Result{SchemaVersion: SchemaVersion, Mode: mode, Reasons: []string{"invalid_mode"}}
	}

	reasons := make([]string, 0, 16)
	if request.ADR0039.Status != "accepted" {
		reasons = append(reasons, "adr_0039_not_accepted")
	} else {
		if request.ADR0039.PlanningOnly {
			reasons = append(reasons, "adr_0039_planning_only")
		}
		if request.ADR0039.AcceptanceID == "" || request.ADR0039.AcceptedAtUnixMS <= 0 {
			reasons = append(reasons, "adr_0039_acceptance_metadata_missing")
		}
	}
	if !request.ADR0113.accepted() {
		reasons = append(reasons, decisionReason("adr_0113", request.ADR0113))
	}
	if !request.ADR0114.accepted() {
		reasons = append(reasons, decisionReason("adr_0114", request.ADR0114))
	}

	evidence := request.Evidence
	if !evidence.CoordinatorOwnerIsolation {
		reasons = append(reasons, "coordinator_owner_isolation_missing")
	}
	if !evidence.DeviceIdentityProof {
		reasons = append(reasons, "device_identity_proof_missing")
	}
	if !evidence.OwnerApprovalAndRevocation {
		reasons = append(reasons, "owner_approval_or_revocation_missing")
	}
	if !evidence.HeartbeatCASAndFreshness {
		reasons = append(reasons, "heartbeat_cas_or_freshness_missing")
	}
	if !evidence.InventoryOwnerScope {
		reasons = append(reasons, "inventory_owner_scope_missing")
	}
	if !evidence.DisabledDefaultAndRouteClose {
		reasons = append(reasons, "disabled_default_or_route_closure_missing")
	}
	if !evidence.SecurityReview {
		reasons = append(reasons, "security_review_missing")
	}

	if mode == ModeExecute || mode == ModeMigrate || mode == ModeFederate {
		if !request.P4.accepted() {
			reasons = append(reasons, decisionReason("p4", request.P4))
		}
		if !evidence.RunnerIsolation {
			reasons = append(reasons, "runner_isolation_missing")
		}
		if !evidence.LeaseFencing {
			reasons = append(reasons, "lease_fencing_missing")
		}
		if !evidence.CancellationAndUncertainWork {
			reasons = append(reasons, "cancellation_or_uncertain_effect_missing")
		}
		if !evidence.VaultArtifactAuthorization {
			reasons = append(reasons, "vault_artifact_authorization_missing")
		}
		if !evidence.AuditOutbox {
			reasons = append(reasons, "audit_outbox_missing")
		}
	}
	if mode == ModeMigrate {
		reasons = append(reasons, "migration_decision_missing")
	}
	if mode == ModeFederate {
		reasons = append(reasons, "federation_decision_missing")
	}

	sort.Strings(reasons)
	return Result{SchemaVersion: SchemaVersion, Mode: mode, Allowed: len(reasons) == 0, Reasons: reasons}
}

func decisionReason(prefix string, decision Decision) string {
	if decision.Status != "accepted" {
		return prefix + "_not_accepted"
	}
	if decision.PlanningOnly {
		return prefix + "_planning_only"
	}
	return prefix + "_acceptance_metadata_missing"
}
