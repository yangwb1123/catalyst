package authenticatedadrlifecyclecontract

import (
	"fmt"

	approvalcontract "forgeos/forge-core/internal/authenticatedadrapprovalcontract"
)

// SignatureChecks returns detached proof-shaped messages. It does not verify
// Ed25519, external roots, currentness, authorization, or lifecycle authority.
func SignatureChecks(bundle *Bundle) ([]SignatureCheck, error) {
	if bundle == nil {
		return nil, fmt.Errorf("bundle is nil")
	}
	node, context, err := validateDocument(bundle.document)
	if err != nil {
		return nil, err
	}
	checks := make([]SignatureCheck, 0)
	state := node["lifecycle_state"].(map[string]any)
	entries := state["ledger"].(map[string]any)["entries"].([]any)
	for index, item := range entries {
		checks, err = appendEntrySignatureChecks(checks, item.(map[string]any), index, context)
		if err != nil {
			return nil, err
		}
	}
	return appendLifecycleCheck(checks, "state", state, "state_sha256", "signature",
		stateSignatureDomain, context.lifecycleRoot)
}

func appendEntrySignatureChecks(checks []SignatureCheck, entry map[string]any,
	index int, context validationContext) ([]SignatureCheck, error) {
	prefix := fmt.Sprintf("entry[%d]", index)
	request := entry["request"].(map[string]any)
	prerequisite := request["acceptance_prerequisite"].(map[string]any)
	var err error
	checks, err = appendApprovalReceiptCheck(checks, prerequisite, prefix, context.approvalRoot)
	if err != nil {
		return nil, err
	}
	checks, err = appendApprovalLedgerCheck(checks, prerequisite, prefix, context.approvalRoot)
	if err != nil {
		return nil, err
	}
	checks, err = appendLifecycleCheck(checks, prefix+".request", request,
		"request_sha256", "signature", requestSignatureDomain, context.lifecycleRoot)
	if err != nil {
		return nil, err
	}
	checks, err = appendLifecycleCheck(checks, prefix+".acceptance",
		entry["acceptance_receipt"].(map[string]any), "acceptance_sha256", "signature",
		acceptanceSignatureDomain, context.lifecycleRoot)
	if err != nil {
		return nil, err
	}
	for receiptIndex, item := range entry["supersession_receipts"].([]any) {
		checks, err = appendLifecycleCheck(checks,
			fmt.Sprintf("%s.supersession[%d]", prefix, receiptIndex), item.(map[string]any),
			"receipt_sha256", "signature", supersessionSignatureDomain, context.lifecycleRoot)
		if err != nil {
			return nil, err
		}
	}
	return checks, nil
}

func appendApprovalReceiptCheck(checks []SignatureCheck, prerequisite map[string]any,
	prefix string, root *approvalcontract.TrustRoot) ([]SignatureCheck, error) {
	raw, err := boundedCanonicalJSON(prerequisite["authorization_receipt"],
		maxAcceptanceBytes, prefix+" approval receipt")
	if err != nil {
		return nil, err
	}
	receipt, err := approvalcontract.DecodeCanonicalReceipt(raw, root)
	if err != nil {
		return nil, err
	}
	approvalChecks, err := approvalcontract.SignatureChecks(receipt)
	if err != nil {
		return nil, err
	}
	for _, check := range approvalChecks {
		checks = append(checks, SignatureCheck{Artifact: prefix + ".approval_receipt",
			ArtifactSHA256: check.ArtifactSHA256, Domain: check.Domain,
			Key: approvalRootKeyView(check.Key), Message: append([]byte(nil), check.Message...),
			Signature: append([]byte(nil), check.Signature...)})
	}
	return checks, nil
}

func appendApprovalLedgerCheck(checks []SignatureCheck, prerequisite map[string]any,
	prefix string, root *approvalcontract.TrustRoot) ([]SignatureCheck, error) {
	signature := prerequisite["authorization_ledger_signature"].(map[string]any)
	key, err := root.ResolveKey(signature["key_id"].(string))
	if err != nil {
		return nil, err
	}
	digest := prerequisite["authorization_ledger_sha256"].(string)
	message, err := signatureMessage(approvalLedgerSignatureDomain, digest)
	if err != nil {
		return nil, err
	}
	proof, err := fixedBase64URL(signature["signature_base64url"],
		prefix+" approval ledger signature", 64)
	if err != nil {
		return nil, err
	}
	return append(checks, SignatureCheck{Artifact: prefix + ".approval_ledger",
		ArtifactSHA256: digest, Domain: approvalLedgerSignatureDomain,
		Key: approvalRootKeyView(key), Message: message, Signature: proof}), nil
}

func appendLifecycleCheck(checks []SignatureCheck, artifact string, node map[string]any,
	digestField, signatureField, domain string, root map[string]any) ([]SignatureCheck, error) {
	digest := node[digestField].(string)
	signature := node[signatureField].(map[string]any)
	keyNode, err := lifecycleRootKeyByID(root, signature["key_id"].(string))
	if err != nil {
		return nil, err
	}
	message, err := signatureMessage(domain, digest)
	if err != nil {
		return nil, err
	}
	proof, err := fixedBase64URL(signature["signature_base64url"], artifact+" signature", 64)
	if err != nil {
		return nil, err
	}
	return append(checks, SignatureCheck{Artifact: artifact, ArtifactSHA256: digest,
		Domain: domain, Key: lifecycleRootKeyView(keyNode), Message: message,
		Signature: proof}), nil
}

func lifecycleRootKeyByID(root map[string]any, keyID string) (map[string]any, error) {
	var match map[string]any
	for _, item := range root["keys"].([]any) {
		key := item.(map[string]any)
		if key["key_id"] == keyID {
			if match != nil {
				return nil, fmt.Errorf("lifecycle root repeats key %q", keyID)
			}
			match = key
		}
	}
	if match == nil {
		return nil, fmt.Errorf("lifecycle root lacks key %q", keyID)
	}
	return match, nil
}

func approvalRootKeyView(key approvalcontract.RootKey) RootKey {
	return RootKey{KeyID: key.KeyID, AuthorityDomain: key.AuthorityDomain,
		PrincipalID: key.PrincipalID, PrincipalType: key.PrincipalType,
		PublicKeyBase64URL: key.PublicKeyBase64URL, Usage: key.Usage}
}
