package authenticatedadrapprovalcontract

import "fmt"

// SignatureChecks returns detached proof-shaped verification inputs for a
// Bundle, AuthorizationInput, Receipt, or Ledger. It does not verify any
// signature.
func SignatureChecks(value any) ([]SignatureCheck, error) {
	switch typed := value.(type) {
	case *Bundle:
		if typed == nil || typed.root == nil {
			return nil, fmt.Errorf("bundle or trust root is nil")
		}
		if _, _, err := validateBundle(typed.document); err != nil {
			return nil, err
		}
		return ledgerSignatureChecks(typed.document["authorization_ledger"].(map[string]any), typed.root.document)
	case *AuthorizationInput:
		return inputSignatureChecks(typed)
	case *Receipt:
		if typed == nil || typed.root == nil {
			return nil, fmt.Errorf("receipt or trust root is nil")
		}
		node, err := validateReceipt(typed.document, typed.root.document)
		if err != nil {
			return nil, err
		}
		return appendSignedCheck(nil, "receipt", node, "receipt_sha256", "signature",
			receiptSignatureDomain, typed.root.document)
	case *Ledger:
		if typed == nil || typed.root == nil {
			return nil, fmt.Errorf("ledger or trust root is nil")
		}
		if _, err := validateLedger(typed.document, typed.root.document); err != nil {
			return nil, err
		}
		return ledgerSignatureChecks(typed.document, typed.root.document)
	default:
		return nil, fmt.Errorf("signature checks do not support %T", value)
	}
}

func inputSignatureChecks(input *AuthorizationInput) ([]SignatureCheck, error) {
	if input == nil || input.root == nil {
		return nil, fmt.Errorf("authorization input or trust root is nil")
	}
	snapshot, err := inputSnapshot(input)
	if err != nil {
		return nil, err
	}
	if _, err = validatePolicy(input.policy, input.root.document, input.metadata); err != nil {
		return nil, err
	}
	if _, err = validateRequest(input.request, input.root.document, input.policy, snapshot); err != nil {
		return nil, err
	}
	checks := make([]SignatureCheck, 0)
	checks, err = appendSignedCheck(checks, "policy", input.policy, "policy_sha256",
		"signature", policySignatureDomain, input.root.document)
	if err != nil {
		return nil, err
	}
	for index, item := range input.snapshots {
		checks, err = appendSignedCheck(checks, fmt.Sprintf("revocation_snapshot[%d]", index),
			item.(map[string]any), "revocation_sha256", "signature", revocationSignatureDomain,
			input.root.document)
		if err != nil {
			return nil, err
		}
	}
	checks, err = appendApprovalChecks(checks, input.request, input.root.document, "request")
	if err != nil {
		return nil, err
	}
	return appendSignedCheck(checks, "request", input.request, "request_sha256",
		"signature", requestSignatureDomain, input.root.document)
}

func ledgerSignatureChecks(ledger, root map[string]any) ([]SignatureCheck, error) {
	checks := make([]SignatureCheck, 0)
	var err error
	for index, item := range ledger["revocation_snapshots"].([]any) {
		checks, err = appendSignedCheck(checks, fmt.Sprintf("revocation_snapshot[%d]", index),
			item.(map[string]any), "revocation_sha256", "signature", revocationSignatureDomain, root)
		if err != nil {
			return nil, err
		}
	}
	for index, item := range ledger["entries"].([]any) {
		checks, err = appendEntryChecks(checks, item.(map[string]any), index, root)
		if err != nil {
			return nil, err
		}
	}
	return appendSignedCheck(checks, "ledger", ledger, "ledger_sha256", "signature",
		ledgerSignatureDomain, root)
}

func appendEntryChecks(checks []SignatureCheck, entry map[string]any, index int,
	root map[string]any) ([]SignatureCheck, error) {
	prefix := fmt.Sprintf("entry[%d]", index)
	var err error
	checks, err = appendSignedCheck(checks, prefix+".policy", entry["policy"].(map[string]any),
		"policy_sha256", "signature", policySignatureDomain, root)
	if err != nil {
		return nil, err
	}
	request := entry["request"].(map[string]any)
	checks, err = appendApprovalChecks(checks, request, root, prefix+".request")
	if err != nil {
		return nil, err
	}
	checks, err = appendSignedCheck(checks, prefix+".request", request, "request_sha256",
		"signature", requestSignatureDomain, root)
	if err != nil {
		return nil, err
	}
	return appendSignedCheck(checks, prefix+".receipt", entry["receipt"].(map[string]any),
		"receipt_sha256", "signature", receiptSignatureDomain, root)
}

func appendApprovalChecks(checks []SignatureCheck, request, root map[string]any,
	prefix string) ([]SignatureCheck, error) {
	for index, item := range request["approval_records"].([]any) {
		record := item.(map[string]any)
		proof := record["authority_proof"].(map[string]any)
		check, err := proofCheck(fmt.Sprintf("%s.approval_record[%d].authority", prefix, index),
			record["approval_sha256"].(string), approvalRecordSignatureDomain,
			proof["key_id"].(string), proof["proof_base64url"].(string), root)
		if err != nil {
			return nil, err
		}
		checks = append(checks, check)
		sod := record["separation_of_duty"].(map[string]any)
		check, err = proofCheck(fmt.Sprintf("%s.approval_record[%d].separation_of_duty", prefix, index),
			record["approval_sha256"].(string), approvalRecordSoDSignatureDomain,
			proof["key_id"].(string), sod["proof_base64url"].(string), root)
		if err != nil {
			return nil, err
		}
		checks = append(checks, check)
	}
	return checks, nil
}

func appendSignedCheck(checks []SignatureCheck, artifact string, node map[string]any,
	digestField, signatureField, domain string, root map[string]any) ([]SignatureCheck, error) {
	digest, ok := node[digestField].(string)
	signature, signatureOK := node[signatureField].(map[string]any)
	if !ok || !signatureOK {
		return nil, fmt.Errorf("%s has no digest or signature", artifact)
	}
	check, err := proofCheck(artifact, digest, domain, signature["key_id"].(string),
		signature["signature_base64url"].(string), root)
	if err != nil {
		return nil, err
	}
	return append(checks, check), nil
}

func proofCheck(artifact, digest, domain, keyID, signature string,
	root map[string]any) (SignatureCheck, error) {
	key, err := keyNodeByID(root, keyID)
	if err != nil {
		return SignatureCheck{}, err
	}
	message, err := signatureMessage(domain, digest)
	if err != nil {
		return SignatureCheck{}, err
	}
	proof, err := fixedBase64URL(signature, artifact+" signature", 64)
	if err != nil {
		return SignatureCheck{}, err
	}
	return SignatureCheck{Artifact: artifact, ArtifactSHA256: digest, Domain: domain,
		Key: rootKeyView(key), Message: append([]byte(nil), message...),
		Signature: append([]byte(nil), proof...)}, nil
}
