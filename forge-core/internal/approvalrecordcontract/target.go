package approvalrecordcontract

import "fmt"

var targetKeys = []string{
	"approver", "authority_binding", "bindings", "conditions", "decision",
	"risk_acceptance_refs", "scope", "separation_of_duty_declaration", "subject",
}

func declaredTarget(record map[string]any) (map[string]any, error) {
	if err := validateRecord(record, false); err != nil {
		return nil, err
	}
	proof := record["authority_proof"].(map[string]any)
	sod := record["separation_of_duty"].(map[string]any)
	authority := make(map[string]any, len(authorityBindingKeys))
	for _, key := range authorityBindingKeys {
		authority[key] = cloneValue(proof[key])
	}
	declaration := make(map[string]any, len(sodDeclarationKeys))
	for _, key := range sodDeclarationKeys {
		declaration[key] = cloneValue(sod[key])
	}
	target := map[string]any{
		"approver": cloneValue(record["approver"]), "authority_binding": authority,
		"bindings": cloneValue(record["bindings"]), "conditions": cloneValue(record["conditions"]),
		"decision": record["decision"], "risk_acceptance_refs": cloneValue(record["risk_acceptance_refs"]),
		"scope": cloneValue(record["scope"]), "separation_of_duty_declaration": declaration,
		"subject": cloneValue(record["subject"]),
	}
	if err := validateTarget(target); err != nil {
		return nil, err
	}
	return target, nil
}

func validateTarget(target map[string]any) error {
	if err := requireKeys(target, targetKeys...); err != nil {
		return fmt.Errorf("declared target: %w", err)
	}
	if err := validateCanonicalByteLimit(target, maxTargetBytes, "declared target"); err != nil {
		return err
	}
	approver, approverErr := objectValue(target, "approver")
	authority, authorityErr := objectValue(target, "authority_binding")
	bindings, bindingsErr := objectValue(target, "bindings")
	scope, scopeErr := objectValue(target, "scope")
	sod, sodErr := objectValue(target, "separation_of_duty_declaration")
	subject, subjectErr := objectValue(target, "subject")
	conditions, conditionErr := arrayValue(target, "conditions")
	risks, riskErr := arrayValue(target, "risk_acceptance_refs")
	decision, decisionErr := stringValue(target, "decision")
	if approverErr != nil || authorityErr != nil || bindingsErr != nil || scopeErr != nil ||
		sodErr != nil || subjectErr != nil || conditionErr != nil || riskErr != nil || decisionErr != nil {
		return fmt.Errorf("declared target nested field type is invalid")
	}
	validators := []func() error{
		func() error { return validatePrincipal(approver, "declared target.approver", true) },
		func() error { return validateAuthorityBinding(authority, "declared target.authority_binding") },
		func() error { return validateBindings(bindings, "declared target.bindings") },
		func() error { return validateConditions(conditions, "declared target.conditions") },
		func() error {
			return validateEnum(decision, "declared target.decision", "abstain", "approve", "reject")
		},
		func() error { return validateRiskRefs(risks, "declared target.risk_acceptance_refs") },
		func() error { return validateScope(scope, "declared target.scope") },
		func() error { return validateSoDDeclaration(sod, "declared target.separation_of_duty_declaration") },
		func() error { return validatePrincipal(subject, "declared target.subject", false) },
	}
	for _, validator := range validators {
		if err := validator(); err != nil {
			return err
		}
	}
	if err := validateSoDConsistency(sod, approver, subject, scope, risks); err != nil {
		return err
	}
	source := authority["authority_source"].(map[string]any)
	return validateProductionFields(decision, scope, source)
}

func targetDigest(target map[string]any) (string, error) {
	if err := validateTarget(target); err != nil {
		return "", err
	}
	return digestNode(targetDomain, target)
}
