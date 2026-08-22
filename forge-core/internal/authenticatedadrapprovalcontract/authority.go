package authenticatedadrapprovalcontract

import (
	"fmt"
)

var keyUsages = []string{
	"approval_authorization_state_sign",
	"approval_policy_sign",
	"approval_request_auth",
	"approval_revocation_sign",
	"architecture_approval_sign",
}

var signatureProfileConstants = map[string]string{
	"algorithm":           "Ed25519",
	"api_version":         signatureProfileAPI,
	"canonicalization":    canonicalization,
	"digest_algorithm":    "SHA-256",
	"kind":                "SignatureProfile",
	"message_preimage":    "domain_separator_utf8_nul_then_raw_32_byte_sha256_digest",
	"profile_id":          signatureProfileID,
	"public_key_encoding": "base64url_unpadded_32_bytes",
	"signature_encoding":  "base64url_unpadded_64_bytes",
}

func signatureProfileDocument() map[string]any {
	node := make(map[string]any, len(signatureProfileConstants)+1)
	for field, value := range signatureProfileConstants {
		node[field] = value
	}
	node["profile_sha256"] = signatureProfileSHA256Pin
	return node
}

func validateSignatureProfile(value any) (map[string]any, error) {
	fields := []string{"algorithm", "api_version", "canonicalization", "digest_algorithm",
		"kind", "message_preimage", "profile_id", "profile_sha256",
		"public_key_encoding", "signature_encoding"}
	node, err := requireKeys(value, "SignatureProfile", fields...)
	if err != nil {
		return nil, err
	}
	if _, err = boundedCanonicalJSON(node, maxProfileBytes, "SignatureProfile"); err != nil {
		return nil, err
	}
	for field, expected := range signatureProfileConstants {
		if node[field] != expected {
			return nil, fmt.Errorf("SignatureProfile.%s drifted from v1", field)
		}
	}
	digest, err := signatureProfileSHA256(node)
	if err != nil || node["profile_sha256"] != digest || digest != signatureProfileSHA256Pin {
		return nil, fmt.Errorf("SignatureProfile self digest does not match")
	}
	return node, nil
}

func validateTrustRoot(value any, profileHash string) (map[string]any, error) {
	label := "ArchitectureDecisionApprovalTrustRoot"
	fields := []string{"api_version", "canonicalization", "keys", "kind", "profile_id",
		"root_sha256", "signature_profile_sha256", "trust_domain", "trust_epoch"}
	node, err := requireKeys(value, label, fields...)
	if err != nil {
		return nil, err
	}
	if _, err = boundedCanonicalJSON(node, maxRootBytes, label); err != nil {
		return nil, err
	}
	if node["api_version"] != trustRootAPI || node["canonicalization"] != canonicalization ||
		node["kind"] != label || node["profile_id"] != profileID {
		return nil, fmt.Errorf("%s envelope drifted from v1", label)
	}
	if _, err = textValue(node["trust_domain"], label+".trust_domain", 160); err != nil {
		return nil, err
	}
	if _, err = intValue(node["trust_epoch"], label+".trust_epoch", 1, maxInt64); err != nil {
		return nil, err
	}
	if node["signature_profile_sha256"] != profileHash {
		return nil, fmt.Errorf("%s does not bind the SignatureProfile", label)
	}
	if err = validateRootKeys(node["keys"]); err != nil {
		return nil, err
	}
	digest, err := trustRootSHA256(node)
	if err != nil || node["root_sha256"] != digest {
		return nil, fmt.Errorf("%s self digest does not match", label)
	}
	return node, nil
}

func validateRootKeys(value any) error {
	items, err := sortedUniqueNodes(value, "trust_root.keys", 6, 20)
	if err != nil {
		return err
	}
	keys := make([]map[string]any, len(items))
	for index, item := range items {
		keys[index], err = validateRootKey(item, fmt.Sprintf("trust_root.keys[%d]", index))
		if err != nil {
			return err
		}
	}
	if err = validateUsageCounts(keys); err != nil {
		return err
	}
	return validateKeyDistinctness(keys)
}

func validateRootKey(value any, label string) (map[string]any, error) {
	node, err := requireKeys(value, label, "key_id", "principal", "public_key_base64url", "usage")
	if err != nil {
		return nil, err
	}
	if _, err = textValue(node["key_id"], label+".key_id", 160); err != nil {
		return nil, err
	}
	usage, err := enumValue(node["usage"], label+".usage", keyUsages...)
	if err != nil {
		return nil, err
	}
	allowed := []string{"service"}
	if usage == "architecture_approval_sign" {
		allowed = []string{"human", "operator"}
	} else if usage == "approval_request_auth" {
		allowed = []string{"agent", "human", "operator", "service"}
	}
	if _, err = validatePrincipal(node["principal"], label+".principal", allowed...); err != nil {
		return nil, err
	}
	if _, err = fixedBase64URL(node["public_key_base64url"], label+".public_key_base64url", 32); err != nil {
		return nil, err
	}
	return node, nil
}

func validateUsageCounts(keys []map[string]any) error {
	counts := make(map[string]int)
	for _, key := range keys {
		counts[key["usage"].(string)]++
	}
	for _, usage := range []string{"approval_authorization_state_sign", "approval_policy_sign",
		"approval_request_auth", "approval_revocation_sign"} {
		if counts[usage] != 1 {
			return fmt.Errorf("trust root service/request key usage counts drifted from v1")
		}
	}
	if counts["architecture_approval_sign"] < 2 || counts["architecture_approval_sign"] > 16 {
		return fmt.Errorf("trust root requires 2..16 architecture approval keys")
	}
	return nil
}

func validateKeyDistinctness(keys []map[string]any) error {
	ids, principals, publicKeys := map[string]bool{}, map[string]bool{}, map[string]bool{}
	for _, key := range keys {
		principal, err := canonicalJSON(key["principal"])
		if err != nil {
			return err
		}
		id, publicKey := key["key_id"].(string), key["public_key_base64url"].(string)
		if ids[id] || principals[string(principal)] || publicKeys[publicKey] {
			return fmt.Errorf("root key IDs, principals, and public keys must be pairwise distinct")
		}
		ids[id], principals[string(principal)], publicKeys[publicKey] = true, true, true
	}
	return nil
}

func keyNodesForUsage(root map[string]any, usage string) ([]map[string]any, error) {
	if !containsString(keyUsages, usage) {
		return nil, fmt.Errorf("unsupported trust-root key usage %q", usage)
	}
	var result []map[string]any
	for _, item := range root["keys"].([]any) {
		key := item.(map[string]any)
		if key["usage"] == usage {
			result = append(result, key)
		}
	}
	return result, nil
}

func keyNodeForUsage(root map[string]any, usage string) (map[string]any, error) {
	matches, err := keyNodesForUsage(root, usage)
	if err != nil {
		return nil, err
	}
	if len(matches) != 1 {
		return nil, fmt.Errorf("trust root does not contain one %s key", usage)
	}
	return matches[0], nil
}

func keyNodeByID(root map[string]any, keyID string) (map[string]any, error) {
	var match map[string]any
	for _, item := range root["keys"].([]any) {
		key := item.(map[string]any)
		if key["key_id"] == keyID {
			if match != nil {
				return nil, fmt.Errorf("key %q is not unique in trust root", keyID)
			}
			match = key
		}
	}
	if match == nil {
		return nil, fmt.Errorf("key %q is absent from trust root", keyID)
	}
	return match, nil
}

func rootKeyView(node map[string]any) RootKey {
	principal := node["principal"].(map[string]any)
	return RootKey{
		KeyID: node["key_id"].(string), Usage: node["usage"].(string),
		PublicKeyBase64URL: node["public_key_base64url"].(string),
		AuthorityDomain:    principal["authority_domain"].(string),
		PrincipalID:        principal["principal_id"].(string),
		PrincipalType:      principal["principal_type"].(string),
	}
}

// Identity returns the caller-supplied self digest and epoch. Neither is an
// external root pin.
func (root *TrustRoot) Identity() (string, int64) {
	return root.document["root_sha256"].(string), root.document["trust_epoch"].(int64)
}

// ResolveKey returns one detached root key by ID.
func (root *TrustRoot) ResolveKey(keyID string) (RootKey, error) {
	node, err := keyNodeByID(root.document, keyID)
	if err != nil {
		return RootKey{}, err
	}
	return rootKeyView(node), nil
}
