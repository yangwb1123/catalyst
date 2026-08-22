package approvalrecordcontract

import "fmt"

var recordKeys = []string{
	"api_version", "approval_id", "approval_sha256", "approver", "authority_proof",
	"bindings", "canonicalization", "conditions", "decision", "decision_basis",
	"effect_vocabulary_sha256", "kind", "risk_acceptance_refs", "scope",
	"separation_of_duty", "subject", "validity",
}

func validateRecord(record map[string]any, allowEmptyIdentity bool) error {
	if err := requireKeys(record, recordKeys...); err != nil {
		return fmt.Errorf("ApprovalRecord: %w", err)
	}
	if err := validateRecordLiterals(record); err != nil {
		return err
	}
	if err := validateCanonicalByteLimit(record, maxRecordBytes, "ApprovalRecord"); err != nil {
		return err
	}
	if err := validateRecordParts(record); err != nil {
		return err
	}
	if err := validateRecordPolicy(record); err != nil {
		return err
	}
	return validateRecordIdentity(record, allowEmptyIdentity)
}

func validateRecordLiterals(record map[string]any) error {
	literals := map[string]string{
		"api_version": approvalAPI, "canonicalization": canonicalization,
		"effect_vocabulary_sha256": effectVocabularyHash, "kind": recordKind,
	}
	for key, expected := range literals {
		value, err := stringValue(record, key)
		if err != nil || value != expected {
			return fmt.Errorf("%s must equal %q", key, expected)
		}
	}
	decision, err := stringValue(record, "decision")
	if err != nil {
		return err
	}
	return validateEnum(decision, "decision", "abstain", "approve", "reject")
}

func validateRecordParts(record map[string]any) error {
	approver, approverErr := objectValue(record, "approver")
	subject, subjectErr := objectValue(record, "subject")
	proof, proofErr := objectValue(record, "authority_proof")
	bindings, bindingsErr := objectValue(record, "bindings")
	basis, basisErr := objectValue(record, "decision_basis")
	scope, scopeErr := objectValue(record, "scope")
	sod, sodErr := objectValue(record, "separation_of_duty")
	validity, validityErr := objectValue(record, "validity")
	conditions, conditionErr := arrayValue(record, "conditions")
	risks, riskErr := arrayValue(record, "risk_acceptance_refs")
	if approverErr != nil || subjectErr != nil || proofErr != nil || bindingsErr != nil ||
		basisErr != nil || scopeErr != nil || sodErr != nil || validityErr != nil ||
		conditionErr != nil || riskErr != nil {
		return fmt.Errorf("ApprovalRecord nested field type is invalid")
	}
	validators := []func() error{
		func() error { return validatePrincipal(approver, "approver", true) },
		func() error { return validatePrincipal(subject, "subject", false) },
		func() error { return validateAuthorityProof(proof) },
		func() error { return validateBindings(bindings, "bindings") },
		func() error { return validateDecisionBasis(basis) },
		func() error { return validateScope(scope, "scope") },
		func() error { return validateConditions(conditions, "conditions") },
		func() error { return validateRiskRefs(risks, "risk_acceptance_refs") },
		func() error { return validateSoD(sod, record) },
		func() error { return validateValidity(validity) },
	}
	for _, validator := range validators {
		if err := validator(); err != nil {
			return err
		}
	}
	return nil
}

func validateRecordPolicy(record map[string]any) error {
	proof := record["authority_proof"].(map[string]any)
	return validateProductionFields(record["decision"], record["scope"].(map[string]any),
		proof["authority_source"].(map[string]any))
}

func validateProductionFields(decision any, scope, source map[string]any) error {
	if scope["scope_type"] != "effect" || scope["environment_class"] != "production" ||
		decision != "approve" {
		return nil
	}
	if scope["effect_id"] != "migration.apply" && scope["effect_id"] != "release.execute" {
		return nil
	}
	if source["authority_class"] != "external_operator" {
		return fmt.Errorf("production apply/execute approval requires external_operator")
	}
	return nil
}

func validateRecordIdentity(record map[string]any, allowEmpty bool) error {
	identifier, idErr := stringValue(record, "approval_id")
	claimed, hashErr := stringValue(record, "approval_sha256")
	if idErr != nil || hashErr != nil {
		return fmt.Errorf("ApprovalRecord identity fields are invalid")
	}
	if allowEmpty && identifier == "" && claimed == "" {
		return nil
	}
	if validateHash(claimed, "approval_sha256") != nil || identifier != "approval-record-"+claimed {
		return fmt.Errorf("ApprovalRecord identity does not match its digest")
	}
	computed, err := approvalDigest(record)
	if err != nil || computed != claimed {
		return fmt.Errorf("ApprovalRecord self digest does not match")
	}
	return nil
}

func approvalDigest(record map[string]any) (string, error) {
	preimage := cloneNode(record)
	preimage["approval_id"] = ""
	preimage["approval_sha256"] = ""
	preimage["authority_proof"].(map[string]any)["proof_base64url"] = ""
	preimage["separation_of_duty"].(map[string]any)["proof_base64url"] = ""
	return digestNode(approvalDomain, preimage)
}
