package devicefabricgate

// This file defines the review-only wire consumed before a future inventory
// or observation activation.  It is intentionally separate from Request:
// Request is the small policy value used by the startup gate, while
// ReviewRequest carries bounded, human-reviewable evidence declarations.  A
// ReviewRequest is never a credential, a route configuration, or proof that a
// device exists.

import (
	"bytes"
	"encoding/json"
	"fmt"
	"io"
	"sort"
	"strings"
	"unicode/utf8"
)

const (
	// ReviewSchemaVersion identifies the review-only activation request wire.
	ReviewSchemaVersion = "forge.device-fabric-activation-request/v1"
	// ReviewCanonicalization is the exact compact JSON spelling used by this
	// package.  It is not an ADR acceptance or an authorization mechanism.
	ReviewCanonicalization = "forgeos.canonical-json/v1"
	// MaxReviewRequestBytes bounds input before decoding nested values.
	MaxReviewRequestBytes = 64 << 10
	// MaxReviewReferenceBytes bounds a reviewer or artifact reference.
	MaxReviewReferenceBytes = 256
)

// ReviewRequest is a review-only declaration for the inventory/observation
// stages.  ProductionAuthorization and every Authority field must remain
// false.  This type has no fields for credentials, device identifiers, routes,
// listeners, placement, or task payloads by design.
type ReviewRequest struct {
	APIVersion              string          `json:"api_version"`
	Canonicalization        string          `json:"canonicalization"`
	RequestID               string          `json:"request_id"`
	ReviewContext           string          `json:"review_context"`
	RequestedMode           Mode            `json:"requested_mode"`
	ProductionAuthorization bool            `json:"production_authorization"`
	Decisions               ReviewDecisions `json:"decisions"`
	Evidence                ReviewEvidence  `json:"evidence"`
	Authority               ReviewAuthority `json:"authority"`
}

// ReviewDecisions is the minimum lifecycle set required for inventory or
// observation.  ADR-0039 remains the route/fabric policy root; ADR-0113 and
// ADR-0114 are separate decisions and cannot be inferred from it.
type ReviewDecisions struct {
	ADR0039 DecisionReview `json:"adr_0039"`
	ADR0113 DecisionReview `json:"adr_0113"`
	ADR0114 DecisionReview `json:"adr_0114"`
}

// DecisionReview mirrors the acceptance metadata that a reviewer must inspect
// without pretending that a JSON declaration can change an ADR lifecycle.
type DecisionReview struct {
	Status           string  `json:"status"`
	AcceptanceID     *string `json:"acceptance_id"`
	AcceptedAtUnixMS *int64  `json:"accepted_at_unix_ms"`
	PlanningOnly     bool    `json:"planning_only"`
}

// ReviewEvidence lists one bounded declaration per evidence boundary.  A
// "verified" claim is a review input, not a cryptographic attestation; the
// gate still has no I/O and cannot manufacture the referenced artifact.
type ReviewEvidence struct {
	CoordinatorOwnerIsolation    EvidenceReview `json:"coordinator_owner_isolation"`
	DeviceIdentityProof          EvidenceReview `json:"device_identity_proof"`
	OwnerApprovalAndRevocation   EvidenceReview `json:"owner_approval_and_revocation"`
	HeartbeatCASAndFreshness     EvidenceReview `json:"heartbeat_cas_and_freshness"`
	InventoryOwnerScope          EvidenceReview `json:"inventory_owner_scope"`
	DisabledDefaultAndRouteClose EvidenceReview `json:"disabled_default_and_route_close"`
	SecurityReview               EvidenceReview `json:"security_review"`
}

// EvidenceReview is deliberately reference-only.  The artifact and reviewer
// strings are bounded IDs/paths, never embedded credentials or evidence
// payloads.  IndependentReview must be true before a claim can be marked
// verified.
type EvidenceReview struct {
	Status            string   `json:"status"`
	ArtifactRefs      []string `json:"artifact_refs"`
	ReviewerRefs      []string `json:"reviewer_refs"`
	IndependentReview bool     `json:"independent_review"`
}

// ReviewAuthority makes accidental authority expansion visible in a fixture
// and fails closed if any future caller flips a boundary bit.
type ReviewAuthority struct {
	OpensListener      bool `json:"opens_listener"`
	RegistersDevices   bool `json:"registers_devices"`
	PersistsInventory  bool `json:"persists_inventory"`
	SelectsPlacement   bool `json:"selects_placement"`
	ReservesResources  bool `json:"reserves_resources"`
	DispatchesTasks    bool `json:"dispatches_tasks"`
	ExecutesRemoteWork bool `json:"executes_remote_work"`
}

// ReviewEvaluation combines structural review validation with the existing
// staged policy. Valid is true only when the request is structurally valid and
// the requested review stage passes every current gate. A true value still
// grants no route or production authority and changes no persistent state.
type ReviewEvaluation struct {
	Valid  bool
	Issues []string
	Result Result
}

// DecodeReviewRequest accepts only one exact compact canonical JSON object.
// Duplicate keys, unknown fields, trailing values, non-canonical whitespace,
// and invalid UTF-8 are rejected before semantic validation.
func DecodeReviewRequest(data []byte) (*ReviewRequest, error) {
	if len(data) == 0 || len(data) > MaxReviewRequestBytes {
		return nil, fmt.Errorf("activation review request exceeds %d bytes or is empty", MaxReviewRequestBytes)
	}
	if !utf8.Valid(data) {
		return nil, fmt.Errorf("activation review request is not valid UTF-8")
	}
	if err := rejectDuplicateKeys(data); err != nil {
		return nil, fmt.Errorf("activation review request JSON: %w", err)
	}
	if err := requireReviewShape(data); err != nil {
		return nil, fmt.Errorf("activation review request shape: %w", err)
	}
	decoder := json.NewDecoder(bytes.NewReader(data))
	decoder.DisallowUnknownFields()
	var request ReviewRequest
	if err := decoder.Decode(&request); err != nil {
		return nil, fmt.Errorf("activation review request JSON: %w", err)
	}
	if err := decoder.Decode(&struct{}{}); err != io.EOF {
		if err == nil {
			return nil, fmt.Errorf("activation review request has trailing JSON value")
		}
		return nil, fmt.Errorf("activation review request has trailing JSON: %w", err)
	}
	canonical, err := json.Marshal(request)
	if err != nil {
		return nil, fmt.Errorf("activation review request canonicalization: %w", err)
	}
	if !bytes.Equal(data, canonical) {
		return nil, fmt.Errorf("activation review request is not exact compact canonical JSON")
	}
	if issues := ValidateReviewRequest(request); len(issues) > 0 {
		return nil, fmt.Errorf("activation review request invalid: %s", strings.Join(issues, ","))
	}
	return &request, nil
}

// CanonicalReviewRequestJSON validates and returns the exact compact JSON
// spelling used by DecodeReviewRequest.  It is useful for review fixtures and
// does not persist or publish the request.
func CanonicalReviewRequestJSON(request ReviewRequest) ([]byte, error) {
	if issues := ValidateReviewRequest(request); len(issues) > 0 {
		return nil, fmt.Errorf("activation review request invalid: %s", strings.Join(issues, ","))
	}
	encoded, err := json.Marshal(request)
	if err != nil {
		return nil, fmt.Errorf("activation review request canonicalization: %w", err)
	}
	return append([]byte(nil), encoded...), nil
}

// ValidateReviewRequest returns deterministic machine-readable issue codes.
// It is pure and intentionally does not check that any referenced file,
// reviewer, ADR, or evidence artifact exists.
func ValidateReviewRequest(request ReviewRequest) []string {
	issues := make([]string, 0, 24)
	if request.APIVersion != ReviewSchemaVersion {
		issues = append(issues, "review_api_version_mismatch")
	}
	if request.Canonicalization != ReviewCanonicalization {
		issues = append(issues, "review_canonicalization_mismatch")
	}
	if !safeReference(request.RequestID) {
		issues = append(issues, "review_request_id_invalid")
	}
	if request.ReviewContext != "repository_snapshot" && request.ReviewContext != "synthetic_review" {
		issues = append(issues, "review_context_invalid")
	}
	if request.RequestedMode != ModeInventory && request.RequestedMode != ModeObserve {
		issues = append(issues, "review_mode_not_inventory_or_observe")
	}
	if request.ProductionAuthorization {
		issues = append(issues, "production_authorization_must_be_false")
	}
	if request.Authority != (ReviewAuthority{}) {
		issues = append(issues, "review_authority_boundary_enabled")
	}
	issues = append(issues, validateDecision("adr_0039", request.Decisions.ADR0039)...)
	issues = append(issues, validateDecision("adr_0113", request.Decisions.ADR0113)...)
	issues = append(issues, validateDecision("adr_0114", request.Decisions.ADR0114)...)
	issues = append(issues, validateEvidence("coordinator_owner_isolation", request.Evidence.CoordinatorOwnerIsolation)...)
	issues = append(issues, validateEvidence("device_identity_proof", request.Evidence.DeviceIdentityProof)...)
	issues = append(issues, validateEvidence("owner_approval_and_revocation", request.Evidence.OwnerApprovalAndRevocation)...)
	issues = append(issues, validateEvidence("heartbeat_cas_and_freshness", request.Evidence.HeartbeatCASAndFreshness)...)
	issues = append(issues, validateEvidence("inventory_owner_scope", request.Evidence.InventoryOwnerScope)...)
	issues = append(issues, validateEvidence("disabled_default_and_route_close", request.Evidence.DisabledDefaultAndRouteClose)...)
	issues = append(issues, validateEvidence("security_review", request.Evidence.SecurityReview)...)
	sort.Strings(issues)
	return uniqueStrings(issues)
}

// EvaluateReview is a pure convenience wrapper for review tooling.  Invalid
// review shape can never be allowed, even if a hand-built Request would pass
// the staged activation policy.
func EvaluateReview(request ReviewRequest) ReviewEvaluation {
	issues := ValidateReviewRequest(request)
	activation := Evaluate(request.activationRequest())
	if len(issues) != 0 {
		activation.Allowed = false
		activation.Reasons = append(activation.Reasons, "review_request_invalid")
		sort.Strings(activation.Reasons)
	}
	return ReviewEvaluation{Valid: len(issues) == 0 && activation.Allowed, Issues: issues, Result: activation}
}

func (request ReviewRequest) activationRequest() Request {
	return Request{
		Mode:    request.RequestedMode,
		ADR0039: request.Decisions.ADR0039.activationDecision(),
		ADR0113: request.Decisions.ADR0113.activationDecision(),
		ADR0114: request.Decisions.ADR0114.activationDecision(),
		Evidence: Evidence{
			CoordinatorOwnerIsolation:    request.Evidence.CoordinatorOwnerIsolation.verified(),
			DeviceIdentityProof:          request.Evidence.DeviceIdentityProof.verified(),
			OwnerApprovalAndRevocation:   request.Evidence.OwnerApprovalAndRevocation.verified(),
			HeartbeatCASAndFreshness:     request.Evidence.HeartbeatCASAndFreshness.verified(),
			InventoryOwnerScope:          request.Evidence.InventoryOwnerScope.verified(),
			DisabledDefaultAndRouteClose: request.Evidence.DisabledDefaultAndRouteClose.verified(),
			SecurityReview:               request.Evidence.SecurityReview.verified(),
		},
	}
}

func (decision DecisionReview) activationDecision() Decision {
	var acceptanceID string
	if decision.AcceptanceID != nil {
		acceptanceID = *decision.AcceptanceID
	}
	var acceptedAt int64
	if decision.AcceptedAtUnixMS != nil {
		acceptedAt = *decision.AcceptedAtUnixMS
	}
	return Decision{Status: decision.Status, AcceptanceID: acceptanceID, AcceptedAtUnixMS: acceptedAt, PlanningOnly: decision.PlanningOnly}
}

func (claim EvidenceReview) verified() bool {
	return claim.Status == "verified" && claim.IndependentReview
}

func validateDecision(prefix string, decision DecisionReview) []string {
	issues := make([]string, 0, 4)
	switch decision.Status {
	case "proposed", "accepted", "rejected", "superseded":
	default:
		issues = append(issues, prefix+"_status_invalid")
	}
	if decision.AcceptanceID != nil && !safeReference(*decision.AcceptanceID) {
		issues = append(issues, prefix+"_acceptance_id_invalid")
	}
	if decision.AcceptedAtUnixMS != nil && *decision.AcceptedAtUnixMS <= 0 {
		issues = append(issues, prefix+"_accepted_at_invalid")
	}
	if decision.Status == "accepted" {
		if !decision.PlanningOnly {
			if decision.AcceptanceID == nil || *decision.AcceptanceID == "" {
				issues = append(issues, prefix+"_acceptance_id_required")
			}
			if decision.AcceptedAtUnixMS == nil || *decision.AcceptedAtUnixMS <= 0 {
				issues = append(issues, prefix+"_accepted_at_required")
			}
		}
	}
	return issues
}

func validateEvidence(prefix string, claim EvidenceReview) []string {
	issues := make([]string, 0, 5)
	if claim.ArtifactRefs == nil {
		issues = append(issues, prefix+"_artifact_refs_missing")
	}
	if claim.ReviewerRefs == nil {
		issues = append(issues, prefix+"_reviewer_refs_missing")
	}
	switch claim.Status {
	case "missing":
		if len(claim.ArtifactRefs) != 0 || len(claim.ReviewerRefs) != 0 || claim.IndependentReview {
			issues = append(issues, prefix+"_missing_claim_has_metadata")
		}
	case "candidate", "verified":
		if len(claim.ArtifactRefs) == 0 {
			issues = append(issues, prefix+"_artifact_refs_required")
		}
		if len(claim.ReviewerRefs) == 0 {
			issues = append(issues, prefix+"_reviewer_refs_required")
		}
		if claim.Status == "verified" && !claim.IndependentReview {
			issues = append(issues, prefix+"_independent_review_required")
		}
	default:
		issues = append(issues, prefix+"_status_invalid")
	}
	issues = append(issues, validateReferences(prefix+"_artifact", claim.ArtifactRefs)...)
	issues = append(issues, validateReferences(prefix+"_reviewer", claim.ReviewerRefs)...)
	return issues
}

func validateReferences(prefix string, references []string) []string {
	issues := make([]string, 0)
	if len(references) > 64 {
		issues = append(issues, prefix+"_refs_too_many")
	}
	seen := make(map[string]struct{}, len(references))
	for _, reference := range references {
		if !safeReference(reference) || len(reference) > MaxReviewReferenceBytes {
			issues = append(issues, prefix+"_ref_invalid")
		}
		if _, ok := seen[reference]; ok {
			issues = append(issues, prefix+"_ref_duplicate")
		}
		seen[reference] = struct{}{}
	}
	return issues
}

func safeReference(value string) bool {
	if value == "" || len(value) > MaxReviewReferenceBytes || !utf8.ValidString(value) {
		return false
	}
	for index, character := range value {
		if index == 0 {
			if !((character >= 'a' && character <= 'z') || (character >= 'A' && character <= 'Z') || (character >= '0' && character <= '9')) {
				return false
			}
			continue
		}
		if !((character >= 'a' && character <= 'z') || (character >= 'A' && character <= 'Z') ||
			(character >= '0' && character <= '9') || character == '.' || character == '_' || character == ':' || character == '/' || character == '#' || character == '-') {
			return false
		}
	}
	pathPart := value
	if fragment := strings.IndexByte(pathPart, '#'); fragment >= 0 {
		pathPart = pathPart[:fragment]
	}
	for _, segment := range strings.Split(pathPart, "/") {
		if segment == "" || segment == "." || segment == ".." {
			return false
		}
	}
	return true
}

func uniqueStrings(values []string) []string {
	if len(values) < 2 {
		return values
	}
	result := values[:1]
	for _, value := range values[1:] {
		if value != result[len(result)-1] {
			result = append(result, value)
		}
	}
	return result
}

func requireReviewShape(data []byte) error {
	var root map[string]json.RawMessage
	if err := json.Unmarshal(data, &root); err != nil || root == nil {
		return fmt.Errorf("root must be an object")
	}
	if err := requireObjectFields("root", root,
		"api_version", "canonicalization", "request_id", "review_context", "requested_mode",
		"production_authorization", "decisions", "evidence", "authority"); err != nil {
		return err
	}
	var decisions map[string]json.RawMessage
	if err := json.Unmarshal(root["decisions"], &decisions); err != nil || decisions == nil {
		return fmt.Errorf("decisions must be an object")
	}
	if err := requireObjectFields("decisions", decisions, "adr_0039", "adr_0113", "adr_0114"); err != nil {
		return err
	}
	var evidence map[string]json.RawMessage
	if err := json.Unmarshal(root["evidence"], &evidence); err != nil || evidence == nil {
		return fmt.Errorf("evidence must be an object")
	}
	evidenceNames := []string{
		"coordinator_owner_isolation", "device_identity_proof", "owner_approval_and_revocation",
		"heartbeat_cas_and_freshness", "inventory_owner_scope", "disabled_default_and_route_close",
		"security_review",
	}
	if err := requireObjectFields("evidence", evidence, evidenceNames...); err != nil {
		return err
	}
	for _, name := range evidenceNames {
		var claim map[string]json.RawMessage
		if err := json.Unmarshal(evidence[name], &claim); err != nil || claim == nil {
			return fmt.Errorf("evidence.%s must be an object", name)
		}
		if err := requireObjectFields("evidence."+name, claim, "status", "artifact_refs", "reviewer_refs", "independent_review"); err != nil {
			return err
		}
	}
	var authority map[string]json.RawMessage
	if err := json.Unmarshal(root["authority"], &authority); err != nil || authority == nil {
		return fmt.Errorf("authority must be an object")
	}
	return requireObjectFields("authority", authority,
		"opens_listener", "registers_devices", "persists_inventory", "selects_placement",
		"reserves_resources", "dispatches_tasks", "executes_remote_work")
}

func requireObjectFields(label string, object map[string]json.RawMessage, fields ...string) error {
	for _, field := range fields {
		if _, ok := object[field]; !ok {
			return fmt.Errorf("%s.%s is required", label, field)
		}
	}
	return nil
}

// rejectDuplicateKeys walks tokens without materializing an object, so a
// repeated key cannot be silently overwritten by encoding/json.
func rejectDuplicateKeys(data []byte) error {
	decoder := json.NewDecoder(bytes.NewReader(data))
	decoder.UseNumber()
	if err := walkJSON(decoder); err != nil {
		return err
	}
	if err := decoder.Decode(&struct{}{}); err != io.EOF {
		if err == nil {
			return fmt.Errorf("trailing JSON value")
		}
		return fmt.Errorf("trailing JSON: %w", err)
	}
	return nil
}

func walkJSON(decoder *json.Decoder) error {
	token, err := decoder.Token()
	if err != nil {
		return err
	}
	delimiter, ok := token.(json.Delim)
	if !ok {
		return nil
	}
	switch delimiter {
	case '{':
		seen := map[string]struct{}{}
		for decoder.More() {
			key, err := decoder.Token()
			if err != nil {
				return err
			}
			name, ok := key.(string)
			if !ok {
				return fmt.Errorf("object key is not a string")
			}
			if _, exists := seen[name]; exists {
				return fmt.Errorf("duplicate object key %q", name)
			}
			seen[name] = struct{}{}
			if err := walkJSON(decoder); err != nil {
				return err
			}
		}
		_, err = decoder.Token()
		return err
	case '[':
		for decoder.More() {
			if err := walkJSON(decoder); err != nil {
				return err
			}
		}
		_, err = decoder.Token()
		return err
	default:
		return fmt.Errorf("unexpected JSON delimiter %q", delimiter)
	}
}
