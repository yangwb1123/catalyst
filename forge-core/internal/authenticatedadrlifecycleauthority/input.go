package authenticatedadrlifecycleauthority

import (
	"encoding/base64"
	"fmt"
	"sort"

	"forgeos/forge-core/internal/adrv2"
	approvalauthority "forgeos/forge-core/internal/authenticatedadrapprovalauthority"
)

type preparedInput struct {
	raw          []byte
	request      map[string]any
	source       approvalauthority.AcceptancePrerequisiteSource
	document     *adrv2.Document
	binding      map[string]any
	idempotency  string
	sequence     int64
	observed     int64
	proposalID   string
	proposalHash string
}

func prepareFreshInput(encoded EncodedTransitionInput,
	stored *approvalauthority.StoredAuthorization, material authorityMaterial,
	trust ExternalTrust, config Config) (preparedInput, error) {
	var result preparedInput
	if stored == nil {
		return result, coded(codeAuthorizationRejected, fmt.Errorf("stored authorization is absent"))
	}
	source, err := stored.AcceptancePrerequisite()
	if err != nil {
		return result, coded(codeAuthorizationRejected, err)
	}
	if source.ObservedAtUnixMS != trust.ObservedAtUnixMS ||
		!constantTimeEqual(source.ApprovalTrustRootSHA256, trust.PinnedApprovalTrustRootSHA256) ||
		source.ApprovalTrustEpoch != trust.PinnedApprovalTrustEpoch {
		return result, coded(codeAuthorizationRejected, fmt.Errorf("stored authorization uses different external trust"))
	}
	if !exactBytes(source.SignatureProfileJSON, material.profileRaw) ||
		!exactBytes(source.ApprovalTrustRootJSON, material.approvalRaw) {
		return result, coded(codeAuthorizationRejected, fmt.Errorf("stored authorization authority bytes differ"))
	}
	request, err := decodeRequest(encoded.RequestJSON)
	if err != nil {
		return result, coded(codeInputRejected, err)
	}
	if err = bindRequestToSource(request, source, material, trust); err != nil {
		return result, coded(codeAuthorizationRejected, err)
	}
	result, err = finishPreparedInput(encoded.RequestJSON, request, source)
	if err != nil {
		return preparedInput{}, coded(codeInputRejected, err)
	}
	if err = rejectProposal(result.proposalID, result.proposalHash,
		config.ExtraExcludedProposalBindingSHA256s); err != nil {
		return preparedInput{}, err
	}
	return result, nil
}

func prepareReplayInput(encoded EncodedTransitionInput, material authorityMaterial,
	trust ExternalTrust) (preparedInput, error) {
	var result preparedInput
	request, err := decodeRequest(encoded.RequestJSON)
	if err != nil {
		return result, coded(codeInputRejected, err)
	}
	if err = validateRequestRootAndTime(request, material, trust, false); err != nil {
		return result, coded(codeInputRejected, err)
	}
	if err = verifyRequestSignature(request, material.requestKey); err != nil {
		return result, coded(codeSignatureRejected, err)
	}
	idempotency, err := stringField(request, "idempotency_key")
	if err != nil || !validIdempotency(idempotency) {
		return result, coded(codeInputRejected, fmt.Errorf("invalid idempotency key"))
	}
	sequence, err := intField(request, "expected_next_sequence")
	if err != nil || sequence < 1 {
		return result, coded(codeInputRejected, fmt.Errorf("invalid request sequence"))
	}
	result = preparedInput{raw: cloneBytes(encoded.RequestJSON), request: request,
		idempotency: idempotency, sequence: sequence}
	return result, nil
}

func decodeRequest(raw []byte) (map[string]any, error) {
	value, err := parseCanonicalJSON(raw, maxRequest, "lifecycle transition request")
	if err != nil {
		return nil, err
	}
	request, err := objectValue(value, "lifecycle transition request")
	if err != nil {
		return nil, err
	}
	fields := []string{"acceptance_prerequisite", "api_version", "canonicalization",
		"expected_current_head_set_sha256", "expected_ledger_sha256", "expected_next_sequence",
		"expires_at_unix_ms", "idempotency_key", "kind", "profile_id",
		"proposal_document_base64url", "request_id", "request_sha256", "requested_at_unix_ms",
		"signature", "supersession_targets", "trust_epoch", "trust_root_sha256"}
	if err = requireFields(request, "lifecycle transition request", fields...); err != nil {
		return nil, err
	}
	if request["api_version"] != requestAPI || request["canonicalization"] != canonicalization ||
		request["kind"] != "ArchitectureDecisionLifecycleTransitionRequest" || request["profile_id"] != profileID {
		return nil, fmt.Errorf("lifecycle request envelope drifted")
	}
	digest, err := digestFor("request", request)
	if err != nil || request["request_sha256"] != digest ||
		request["request_id"] != "architecture-decision-lifecycle-request-"+digest {
		return nil, fmt.Errorf("lifecycle request identity differs")
	}
	return request, nil
}

func bindRequestToSource(request map[string]any,
	source approvalauthority.AcceptancePrerequisiteSource,
	material authorityMaterial, trust ExternalTrust) error {
	expected, err := prerequisiteFromSource(source)
	if err != nil {
		return err
	}
	actual, err := objectField(request, "acceptance_prerequisite")
	if err != nil {
		return err
	}
	expectedJSON, err := canonicalJSON(expected, maxRequest, "expected prerequisite")
	if err != nil {
		return err
	}
	actualJSON, err := canonicalJSON(actual, maxRequest, "request prerequisite")
	if err != nil {
		return err
	}
	if !exactBytes(expectedJSON, actualJSON) {
		return fmt.Errorf("request prerequisite differs from opaque stored authorization")
	}
	proposalText, err := stringField(request, "proposal_document_base64url")
	if err != nil {
		return err
	}
	if base64.RawURLEncoding.EncodeToString(source.ProposalDocument) != proposalText {
		return fmt.Errorf("request proposal bytes differ from stored authorization")
	}
	if err = validateRequestRootAndTime(request, material, trust, true); err != nil {
		return err
	}
	return verifyRequestSignature(request, material.requestKey)
}

func prerequisiteFromSource(source approvalauthority.AcceptancePrerequisiteSource) (map[string]any, error) {
	binding, err := parseObjectBytes(source.ProposalBindingJSON, 64*1024, "proposal binding")
	if err != nil {
		return nil, err
	}
	receipt, err := parseObjectBytes(source.AuthorizationReceiptJSON, 256*1024, "authorization receipt")
	if err != nil {
		return nil, err
	}
	ledgerSignature, err := parseObjectBytes(source.AuthorizationLedgerSignatureJSON, 16*1024, "approval ledger signature")
	if err != nil {
		return nil, err
	}
	value := map[string]any{
		"api_version": prerequisiteAPI, "approval_trust_epoch": source.ApprovalTrustEpoch,
		"approval_trust_root_sha256":                    source.ApprovalTrustRootSHA256,
		"authorization_ledger_clock_high_water_unix_ms": source.AuthorizationLedgerClockHighWaterUnixMS,
		"authorization_ledger_last_sequence":            source.AuthorizationLedgerLastSequence,
		"authorization_ledger_sha256":                   source.AuthorizationLedgerSHA256,
		"authorization_ledger_signature":                ledgerSignature, "authorization_receipt": receipt,
		"authorization_receipt_physical_sha256": source.AuthorizationReceiptPhysicalSHA256,
		"canonicalization":                      canonicalization, "kind": "ArchitectureDecisionAcceptancePrerequisite",
		"observed_at_unix_ms": source.ObservedAtUnixMS, "prerequisite_sha256": "",
		"profile_id": profileID, "proposal_binding": binding,
		"revocation_high_water_sequence": source.RevocationHighWaterSequence,
		"revocation_high_water_sha256":   source.RevocationHighWaterSHA256,
	}
	digest, err := digestFor("prerequisite", value)
	if err != nil {
		return nil, err
	}
	value["prerequisite_sha256"] = digest
	return value, nil
}

func parseObjectBytes(raw []byte, maximum int, label string) (map[string]any, error) {
	value, err := parseCanonicalJSON(raw, maximum, label)
	if err != nil {
		return nil, err
	}
	return objectValue(value, label)
}

func validateRequestRootAndTime(request map[string]any, material authorityMaterial,
	trust ExternalTrust, current bool) error {
	if request["trust_root_sha256"] != trust.PinnedLifecycleTrustRootSHA256 ||
		request["trust_epoch"] != trust.PinnedLifecycleTrustEpoch {
		return fmt.Errorf("request does not bind lifecycle external trust")
	}
	requested, err := intField(request, "requested_at_unix_ms")
	if err != nil {
		return err
	}
	expires, err := intField(request, "expires_at_unix_ms")
	if err != nil {
		return err
	}
	if current && (requested != trust.ObservedAtUnixMS || expires <= requested ||
		expires-requested > 300_000 || trust.ObservedAtUnixMS >= expires) {
		return fmt.Errorf("request is not current at exact trusted observation")
	}
	if material.lifecycle["root_sha256"] != request["trust_root_sha256"] {
		return fmt.Errorf("request root differs from protected lifecycle root")
	}
	return nil
}

func finishPreparedInput(raw []byte, request map[string]any,
	source approvalauthority.AcceptancePrerequisiteSource) (preparedInput, error) {
	var result preparedInput
	binding, err := parseObjectBytes(source.ProposalBindingJSON, 64*1024, "proposal binding")
	if err != nil {
		return result, err
	}
	name, err := stringField(binding, "document_name")
	if err != nil {
		return result, err
	}
	document, err := adrv2.ValidateDocument(name, source.ProposalDocument)
	if err != nil {
		return result, err
	}
	proposalID, err := stringField(binding, "adr_id")
	if err != nil || proposalID != document.Frontmatter.ADRID {
		return result, fmt.Errorf("proposal ADR identity differs")
	}
	proposalHash, err := stringField(binding, "proposal_binding_sha256")
	if err != nil || proposalHash != source.ProposalBindingSHA256 {
		return result, fmt.Errorf("proposal binding identity differs")
	}
	if binding["body_sha256"] != document.Frontmatter.BodySHA256 || binding["self_sha256"] != document.Frontmatter.SelfSHA256 ||
		binding["physical_sha256"] != sha256Bytes(source.ProposalDocument) {
		return result, fmt.Errorf("proposal binding differs from immutable source")
	}
	targets, err := targetIDs(request)
	if err != nil || !equalStrings(targets, document.Frontmatter.Supersedes) {
		return result, fmt.Errorf("request targets differ from immutable supersedes")
	}
	idempotency, err := stringField(request, "idempotency_key")
	if err != nil || !validIdempotency(idempotency) {
		return result, fmt.Errorf("invalid idempotency key")
	}
	sequence, err := intField(request, "expected_next_sequence")
	if err != nil || sequence < 1 {
		return result, fmt.Errorf("invalid expected sequence")
	}
	result = preparedInput{raw: cloneBytes(raw), request: request, source: source,
		document: document, binding: binding, idempotency: idempotency, sequence: sequence,
		observed: source.ObservedAtUnixMS, proposalID: proposalID, proposalHash: proposalHash}
	return result, nil
}

func targetIDs(request map[string]any) ([]string, error) {
	targets, err := arrayField(request, "supersession_targets")
	if err != nil || len(targets) > maxTargets {
		return nil, fmt.Errorf("invalid supersession target array")
	}
	result := make([]string, len(targets))
	for index, raw := range targets {
		target, itemErr := objectValue(raw, "target")
		if itemErr != nil {
			return nil, itemErr
		}
		result[index], itemErr = stringField(target, "adr_id")
		if itemErr != nil {
			return nil, itemErr
		}
	}
	expected := append([]string(nil), result...)
	sort.Strings(expected)
	if !equalStrings(result, expected) {
		return nil, fmt.Errorf("targets must be sorted")
	}
	return result, nil
}

func equalStrings(left, right []string) bool {
	if len(left) != len(right) {
		return false
	}
	for index := range left {
		if left[index] != right[index] {
			return false
		}
	}
	return true
}

func rejectProposal(adrID, binding string, extra []string) error {
	if excludedADRIDs[adrID] {
		return coded(codeProposalExcluded, fmt.Errorf("bootstrap lifecycle proposal excluded"))
	}
	for _, value := range extra {
		if constantTimeEqual(value, binding) {
			return coded(codeProposalExcluded, fmt.Errorf("proposal binding excluded"))
		}
	}
	return nil
}
