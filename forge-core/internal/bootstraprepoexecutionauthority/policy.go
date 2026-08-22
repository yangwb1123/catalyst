package bootstraprepoexecutionauthority

import (
	"fmt"

	"forgeos/forge-core/internal/bootstrapgrantauthority"
)

var policyKeys = []string{"activation", "api_version", "bindings", "budget", "canonicalization",
	"capability", "disposition", "effect_id", "execution_policy_id", "execution_policy_sha256",
	"execution_trust_epoch", "execution_trust_root_sha256", "grant_envelope_sha256", "grant_id",
	"grant_issuance_ledger_sequence", "grant_issuance_receipt_sha256", "grant_policy_sha256",
	"grant_request_sha256", "grant_sha256", "idempotency_key", "issuance_trust_epoch",
	"issuance_trust_root_sha256", "kind", "manifest_sha256", "profile_id", "requested_action",
	"requested_action_sha256", "signature", "subject", "task_binding", "validity"}

// Policy is a strict and authenticated execution activation decision.
type Policy struct {
	document map[string]any
	grant    *issuedGrant
}

// DecodePolicy authenticates a Policy against both roots, Grant, and Manifest.
func DecodePolicy(data []byte, trust *Trust, issuance *bootstrapgrantauthority.Ledger,
	manifest *Manifest) (*Policy, error) {
	if trust == nil || issuance == nil || manifest == nil {
		return nil, fmt.Errorf("Trust, issuance Ledger, and Manifest are required")
	}
	document, err := decodeCanonical(data, maxPolicyBytes)
	if err != nil {
		return nil, err
	}
	issued, err := resolveIssuedGrant(document, issuance)
	if err != nil {
		return nil, err
	}
	if err = validatePolicy(document, trust, issued, manifest); err != nil {
		return nil, err
	}
	return &Policy{document: document, grant: issued}, nil
}

func (policy *Policy) canonicalDocument() map[string]any { return cloneDocument(policy.document) }

// AllowsExecution reports the authenticated allow/activate_once pair.
func (policy *Policy) AllowsExecution() bool {
	return policy != nil && policy.document["disposition"] == "allow" &&
		policy.document["activation"] == "activate_once"
}

func resolveIssuedGrant(document map[string]any,
	issuance *bootstrapgrantauthority.Ledger) (*issuedGrant, error) {
	grantID, idErr := stringValue(document, "grant_id")
	grantHash, hashErr := stringValue(document, "grant_sha256")
	envelopeHash, envelopeErr := stringValue(document, "grant_envelope_sha256")
	receiptHash, receiptErr := stringValue(document, "grant_issuance_receipt_sha256")
	sequence, sequenceErr := intValue(document, "grant_issuance_ledger_sequence")
	if idErr != nil || hashErr != nil || envelopeErr != nil || receiptErr != nil || sequenceErr != nil {
		return nil, fmt.Errorf("ExecutionPolicy issued Grant identity is invalid")
	}
	projection, found, err := bootstrapgrantauthority.LookupIssuedGrant(issuance, grantID,
		grantHash, envelopeHash, receiptHash, sequence)
	if err != nil || !found {
		return nil, fmt.Errorf("ExecutionPolicy does not identify an issued Grant")
	}
	return decodeIssuedProjection(projection)
}

func validatePolicy(document map[string]any, trust *Trust, grant *issuedGrant,
	manifest *Manifest) error {
	if err := validateReplayPolicy(document, trust, manifest); err != nil {
		return err
	}
	if err := validateManifestGrant(manifest, grant); err != nil {
		return err
	}
	if err := validatePolicyRelations(document, trust, grant.document, manifest.document); err != nil {
		return err
	}
	return nil
}

func validateReplayPolicy(document map[string]any, trust *Trust, manifest *Manifest) error {
	if err := requireKeys(document, policyKeys...); err != nil {
		return fmt.Errorf("BootstrapRepoReadExecutionPolicy: %w", err)
	}
	if err := validatePolicyShape(document, trust); err != nil {
		return err
	}
	if document["manifest_sha256"] != manifest.document["manifest_sha256"] ||
		!sameCanonical(document["subject"], trust.keys["execution_request_auth"].principal) {
		return fmt.Errorf("ExecutionPolicy manifest or request principal binding is invalid")
	}
	if err := validateManifestAction(manifest, document["requested_action"]); err != nil {
		return err
	}
	return validateSigned(document, "execution_policy_sha256", policyDomain,
		policySignatureDomain, maxPolicyBytes, "ExecutionPolicy", trust, "execution_policy_sign", "")
}

func validatePolicyShape(document map[string]any, trust *Trust) error {
	if err := validateEnvelope(document, policyAPI, "BootstrapRepoReadExecutionPolicy"); err != nil {
		return err
	}
	if document["profile_id"] != profileID || document["effect_id"] != "repo.read" {
		return fmt.Errorf("ExecutionPolicy profile or effect is invalid")
	}
	disposition, dispositionErr := stringValue(document, "disposition")
	activation, activationErr := stringValue(document, "activation")
	if dispositionErr != nil || activationErr != nil ||
		!((disposition == "allow" && activation == "activate_once") ||
			(disposition == "deny" && activation == "do_not_activate")) {
		return fmt.Errorf("ExecutionPolicy disposition and activation are invalid")
	}
	if err := validatePolicyParts(document); err != nil {
		return err
	}
	return validateAuthorityBinding(document, trust, "ExecutionPolicy")
}

func validatePolicyParts(document map[string]any) error {
	if id, err := stringValue(document, "execution_policy_id"); err != nil || validateText(id, "execution_policy_id", 160) != nil {
		return fmt.Errorf("execution_policy_id is invalid")
	}
	key, err := stringValue(document, "idempotency_key")
	if err != nil || !idempotencyPattern.MatchString(key) {
		return fmt.Errorf("ExecutionPolicy idempotency_key is invalid")
	}
	validators := []func() error{
		func() error { return validateBindings(document["bindings"], "ExecutionPolicy bindings") },
		func() error { _, e := validateBudget(document["budget"], "ExecutionPolicy budget"); return e },
		func() error { return validateCapability(document["capability"], "ExecutionPolicy capability") },
		func() error { _, e := validatePrincipal(document["subject"], "ExecutionPolicy subject"); return e },
		func() error { return validateTask(document["task_binding"], "ExecutionPolicy task_binding") },
		func() error {
			return validateRequestedAction(document["requested_action"], document["requested_action_sha256"])
		},
		func() error { return validatePolicyValidity(document["validity"]) },
	}
	for _, validator := range validators {
		if err := validator(); err != nil {
			return err
		}
	}
	for _, field := range []string{"execution_policy_sha256", "execution_trust_root_sha256",
		"grant_envelope_sha256", "grant_issuance_receipt_sha256", "grant_policy_sha256",
		"grant_request_sha256", "grant_sha256", "issuance_trust_root_sha256", "manifest_sha256",
		"requested_action_sha256"} {
		if err := validateHashField(document, field, "ExecutionPolicy "+field); err != nil {
			return err
		}
	}
	sequence, sequenceErr := intValue(document, "grant_issuance_ledger_sequence")
	grantHash, hashErr := stringValue(document, "grant_sha256")
	grantID, idErr := stringValue(document, "grant_id")
	if sequenceErr != nil || sequence < 1 || sequence > 256 || hashErr != nil || idErr != nil ||
		grantID != "capability-grant-"+grantHash {
		return fmt.Errorf("ExecutionPolicy Grant identity or sequence is invalid")
	}
	return nil
}

func validatePolicyValidity(value any) error {
	node, ok := value.(map[string]any)
	if !ok || requireKeys(node, "expires_at_unix_ms", "not_before_unix_ms") != nil {
		return fmt.Errorf("ExecutionPolicy validity fields are invalid")
	}
	start, startErr := intValue(node, "not_before_unix_ms")
	end, endErr := intValue(node, "expires_at_unix_ms")
	if startErr != nil || endErr != nil || start < 0 || end <= start || end-start > maxFreshnessMillis {
		return fmt.Errorf("ExecutionPolicy validity must be ordered within five minutes")
	}
	return nil
}

func validatePolicyRelations(policy map[string]any, trust *Trust, grant,
	manifest map[string]any) error {
	bindings := []string{"bindings", "capability", "grant_envelope_sha256", "grant_id",
		"grant_issuance_ledger_sequence", "grant_issuance_receipt_sha256", "grant_policy_sha256",
		"grant_request_sha256", "grant_sha256", "issuance_trust_epoch", "issuance_trust_root_sha256",
		"subject", "task_binding"}
	for _, field := range bindings {
		if !sameCanonical(policy[field], grant[field]) {
			return fmt.Errorf("ExecutionPolicy field %s differs from issued Grant", field)
		}
	}
	if policy["manifest_sha256"] != manifest["manifest_sha256"] ||
		!sameCanonical(policy["subject"], trust.keys["execution_request_auth"].principal) {
		return fmt.Errorf("ExecutionPolicy manifest or request principal binding is invalid")
	}
	if !sameCanonical(policy["requested_action"].(map[string]any)["resources"], grant["resources"]) {
		return fmt.Errorf("requested_action differs from complete issued Grant path set")
	}
	if !budgetCovers(grant["budget"], policy["budget"]) ||
		validateBudgetCoversAction(policy["budget"], policy["requested_action"]) != nil {
		return fmt.Errorf("ExecutionPolicy budget exceeds Grant or does not cover action")
	}
	grantValidity := grant["validity"].(map[string]any)
	policyValidity := policy["validity"].(map[string]any)
	if policyValidity["not_before_unix_ms"].(int64) < grantValidity["not_before_unix_ms"].(int64) ||
		policyValidity["expires_at_unix_ms"].(int64) > grantValidity["expires_at_unix_ms"].(int64) {
		return fmt.Errorf("ExecutionPolicy validity exceeds Grant validity")
	}
	return nil
}

func budgetCovers(limitValue, requestedValue any) bool {
	limit, limitOK := limitValue.(map[string]any)
	requested, requestedOK := requestedValue.(map[string]any)
	if !limitOK || !requestedOK {
		return false
	}
	for _, field := range budgetKeys {
		maximum, maxErr := intValue(limit, field)
		actual, actualErr := intValue(requested, field)
		if maxErr != nil || actualErr != nil || actual > maximum {
			return false
		}
	}
	return true
}

func validateAuthorityBinding(document map[string]any, trust *Trust, label string) error {
	if document["execution_trust_root_sha256"] != trust.rootHash ||
		document["execution_trust_epoch"] != trust.epoch ||
		document["issuance_trust_root_sha256"] != trust.issuanceRootHash ||
		document["issuance_trust_epoch"] != trust.issuanceEpoch {
		return fmt.Errorf("%s authority binding is invalid", label)
	}
	return nil
}

func validateSigned(document map[string]any, digestField string, digestDomain,
	signatureDomain []byte, maximum int, label string, trust *Trust, usage, idField string) error {
	claimed, err := stringValue(document, digestField)
	if err != nil || validateHash(claimed, label+" digest") != nil {
		return fmt.Errorf("%s self digest is invalid", label)
	}
	computed, err := selfDigest(digestDomain, document, digestField, maximum, label, true, idField)
	if err != nil || computed != claimed {
		return fmt.Errorf("%s self digest does not match", label)
	}
	signature, err := validateSignature(document["signature"], trust, usage, label)
	if err != nil {
		return err
	}
	return verifyDigest(trust.keys[usage].publicKey, signatureDomain, claimed,
		signature["signature_base64url"].(string))
}
