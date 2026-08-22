package capabilitygrantcontract

import "fmt"

var grantKeys = []string{
	"api_version", "approval_refs", "authority_proof", "bindings", "budget", "canonicalization",
	"capability", "effect_vocabulary_sha256", "grant_id", "grant_sha256", "issuance_phase", "kind",
	"scope", "separation_of_duty", "subject", "task_binding", "usage_policy", "validity",
}

func validateGrant(grant map[string]any) error {
	if err := requireKeys(grant, grantKeys...); err != nil {
		return fmt.Errorf("CapabilityGrant: %w", err)
	}
	if err := validateGrantLiterals(grant); err != nil {
		return err
	}
	parts, err := readGrantParts(grant)
	if err != nil {
		return err
	}
	if err := validateGrantParts(parts); err != nil {
		return err
	}
	if err := validateGrantPolicy(grant, parts); err != nil {
		return err
	}
	if err := validateCanonicalByteLimit(grant, maxGrantBytes, "CapabilityGrant"); err != nil {
		return err
	}
	return validateGrantDigest(grant)
}

func validateGrantLiterals(grant map[string]any) error {
	literals := map[string]string{
		"api_version": "forgeos.capability-grant/v1", "canonicalization": "forgeos.canonical-json/v1",
		"kind": "CapabilityGrant", "effect_vocabulary_sha256": frozenVocabularySHA256,
	}
	for key, expected := range literals {
		if err := requireStringLiteral(grant, key, expected); err != nil {
			return err
		}
	}
	phase, err := stringValue(grant, "issuance_phase")
	if err != nil {
		return err
	}
	return validateEnum(phase, "issuance_phase", "bootstrap_planning", "plan_finalization")
}

type grantParts struct {
	proof, bindings, budget, capability map[string]any
	scope, separation, subject          map[string]any
	task, usage, validity               map[string]any
	approvals                           []any
	issuer                              map[string]any
}

func readGrantParts(grant map[string]any) (*grantParts, error) {
	keys := []string{"authority_proof", "bindings", "budget", "capability", "scope", "separation_of_duty",
		"subject", "task_binding", "usage_policy", "validity"}
	objects := make(map[string]map[string]any, len(keys))
	for _, key := range keys {
		value, err := objectValue(grant, key)
		if err != nil {
			return nil, err
		}
		objects[key] = value
	}
	approvals, err := arrayValue(grant, "approval_refs")
	if err != nil {
		return nil, err
	}
	return &grantParts{
		proof: objects["authority_proof"], bindings: objects["bindings"], budget: objects["budget"],
		capability: objects["capability"], scope: objects["scope"], separation: objects["separation_of_duty"],
		subject: objects["subject"], task: objects["task_binding"], usage: objects["usage_policy"],
		validity: objects["validity"], approvals: approvals,
	}, nil
}

func validateGrantParts(parts *grantParts) error {
	issuer, err := validateAuthorityProof(parts.proof)
	if err != nil {
		return fmt.Errorf("authority_proof: %w", err)
	}
	parts.issuer = issuer
	validators := []func() error{
		func() error { return validateBindings(parts.bindings) },
		func() error { return validateBudget(parts.budget) },
		func() error { return validateCapability(parts.capability) },
		func() error { _, scopeErr := validateScope(parts.scope); return scopeErr },
		func() error { return validatePrincipal(parts.subject) },
		func() error { return validateTaskBinding(parts.task) },
		func() error { return validateUsagePolicy(parts.usage, parts.budget) },
		func() error { return validateValidity(parts.validity) },
		func() error { return validateApprovalRefs(parts.approvals) },
	}
	for _, validator := range validators {
		if err := validator(); err != nil {
			return err
		}
	}
	return nil
}

func validateGrantPolicy(grant map[string]any, parts *grantParts) error {
	if err := validatePhaseBindings(grant, parts.bindings); err != nil {
		return err
	}
	if err := validateSeparationOfDuty(parts.separation, parts.issuer, parts.subject); err != nil {
		return err
	}
	if err := validateProductionRestriction(parts.scope, parts.issuer, parts.approvals); err != nil {
		return err
	}
	grantID, idErr := stringValue(grant, "grant_id")
	grantHash, hashErr := stringValue(grant, "grant_sha256")
	if idErr != nil || hashErr != nil || validateHash(grantHash, "grant_sha256") != nil {
		return fmt.Errorf("grant identity fields are invalid")
	}
	if grantID != "capability-grant-"+grantHash {
		return fmt.Errorf("grant_id must be derived from grant_sha256")
	}
	return nil
}

func validatePhaseBindings(grant, bindings map[string]any) error {
	phase, _ := stringValue(grant, "issuance_phase")
	if phase != "plan_finalization" {
		return nil
	}
	for _, key := range []string{"impact_sha256", "plan_sha256", "risk_sha256"} {
		if bindings[key] == nil {
			return fmt.Errorf("plan_finalization requires %s", key)
		}
	}
	return nil
}

func validateGrantDigest(grant map[string]any) error {
	claimed, _ := stringValue(grant, "grant_sha256")
	preimage := cloneNode(grant)
	preimage["grant_id"] = ""
	preimage["grant_sha256"] = ""
	proof, err := objectValue(preimage, "authority_proof")
	if err != nil {
		return err
	}
	proof["proof_base64url"] = ""
	computed, err := digestNode(grantDigestDomain, preimage)
	if err != nil || computed != claimed {
		return fmt.Errorf("grant_sha256 does not match canonical grant preimage")
	}
	return nil
}
