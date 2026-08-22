package authenticatedadrlifecyclecontract

import (
	"fmt"

	approvalcontract "forgeos/forge-core/internal/authenticatedadrapprovalcontract"
)

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

func validateApprovalRoot(value any) (*approvalcontract.TrustRoot, error) {
	raw, err := boundedCanonicalJSON(value, maxGoldenBytes, "approval trust root")
	if err != nil {
		return nil, err
	}
	root, err := approvalcontract.DecodeCanonicalTrustRoot(raw)
	if err != nil {
		return nil, fmt.Errorf("approval trust root: %w", err)
	}
	return root, nil
}

func validateLifecycleRoot(value any, profileHash string) (map[string]any, error) {
	label := "ArchitectureDecisionLifecycleTrustRoot"
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
	if err = validateLifecycleRootScalars(node, profileHash); err != nil {
		return nil, err
	}
	if err = validateLifecycleRootKeys(node); err != nil {
		return nil, err
	}
	digest, err := trustRootSHA256(node)
	if err != nil || node["root_sha256"] != digest {
		return nil, fmt.Errorf("%s self digest does not match", label)
	}
	return node, nil
}

func validateLifecycleRootScalars(node map[string]any, profileHash string) error {
	if _, err := textValue(node["trust_domain"], "lifecycle_root.trust_domain", 160); err != nil {
		return err
	}
	if _, err := intValue(node["trust_epoch"], "lifecycle_root.trust_epoch", 1, maxInt64); err != nil {
		return err
	}
	if node["signature_profile_sha256"] != profileHash {
		return fmt.Errorf("lifecycle trust root does not bind the SignatureProfile")
	}
	_, err := shaValue(node["root_sha256"], "lifecycle_root.root_sha256")
	return err
}

func validateLifecycleRootKeys(root map[string]any) error {
	items, err := sortedUniqueNodes(root["keys"], "lifecycle_root.keys", 2)
	if err != nil {
		return err
	}
	keys := make([]map[string]any, len(items))
	for index, item := range items {
		keys[index], err = validateLifecycleRootKey(item, index)
		if err != nil {
			return err
		}
	}
	if err = validateLifecycleUsages(keys); err != nil {
		return err
	}
	return validateLifecycleKeyDistinctness(keys, root["trust_domain"].(string))
}

func validateLifecycleRootKey(value any, index int) (map[string]any, error) {
	label := fmt.Sprintf("lifecycle_root.keys[%d]", index)
	node, err := requireKeys(value, label, "key_id", "principal", "public_key_base64url", "usage")
	if err != nil {
		return nil, err
	}
	if _, err = textValue(node["key_id"], label+".key_id", 160); err != nil {
		return nil, err
	}
	usage, err := enumValue(node["usage"], label+".usage", requestKeyUsage, stateKeyUsage)
	if err != nil {
		return nil, err
	}
	allowed := []string{"service"}
	if usage == requestKeyUsage {
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

func validateLifecycleUsages(keys []map[string]any) error {
	counts := map[string]int{}
	for _, key := range keys {
		counts[key["usage"].(string)]++
	}
	if counts[requestKeyUsage] != 1 || counts[stateKeyUsage] != 1 {
		return fmt.Errorf("lifecycle trust root requires one key for each exact usage")
	}
	return nil
}

func validateLifecycleKeyDistinctness(keys []map[string]any, domain string) error {
	ids, publicKeys, principals := map[string]bool{}, map[string]bool{}, map[string]bool{}
	for _, key := range keys {
		principal := key["principal"].(map[string]any)
		if principal["authority_domain"] != domain {
			return fmt.Errorf("lifecycle key principals must use the root trust domain")
		}
		identity := principalIdentity(principal)
		id, publicKey := key["key_id"].(string), key["public_key_base64url"].(string)
		if ids[id] || publicKeys[publicKey] || principals[identity] {
			return fmt.Errorf("lifecycle key IDs, public keys, and principals must differ")
		}
		ids[id], publicKeys[publicKey], principals[identity] = true, true, true
	}
	return nil
}

func validateIndependentRoots(lifecycle map[string]any, approval *approvalcontract.TrustRoot) error {
	facts, err := approvalcontract.Facts(approval)
	if err != nil {
		return err
	}
	if lifecycle["root_sha256"] == facts.TrustRootSHA256 ||
		lifecycle["trust_domain"] == facts.TrustDomain {
		return fmt.Errorf("approval and lifecycle roots/domains must be independent")
	}
	return compareIndependentKeys(lifecycle["keys"].([]any), facts.RootKeys)
}

func compareIndependentKeys(lifecycle []any, approval []approvalcontract.RootKey) error {
	ids, publicKeys, principals := map[string]bool{}, map[string]bool{}, map[string]bool{}
	for _, key := range approval {
		ids[key.KeyID], publicKeys[key.PublicKeyBase64URL] = true, true
		principals[approvalPrincipalIdentity(key)] = true
	}
	for _, item := range lifecycle {
		key := item.(map[string]any)
		if ids[key["key_id"].(string)] || publicKeys[key["public_key_base64url"].(string)] {
			return fmt.Errorf("approval and lifecycle roots reuse key identity")
		}
		if principals[principalIdentity(key["principal"].(map[string]any))] {
			return fmt.Errorf("approval and lifecycle roots reuse a principal")
		}
	}
	return nil
}

func principalIdentity(node map[string]any) string {
	return node["authority_domain"].(string) + "\x00" + node["principal_id"].(string) +
		"\x00" + node["principal_type"].(string)
}

func approvalPrincipalIdentity(key approvalcontract.RootKey) string {
	return key.AuthorityDomain + "\x00" + key.PrincipalID + "\x00" + key.PrincipalType
}

func lifecycleKey(root map[string]any, usage string) (map[string]any, error) {
	var match map[string]any
	for _, item := range root["keys"].([]any) {
		key := item.(map[string]any)
		if key["usage"] == usage {
			if match != nil {
				return nil, fmt.Errorf("lifecycle root repeats usage %q", usage)
			}
			match = key
		}
	}
	if match == nil {
		return nil, fmt.Errorf("lifecycle root lacks usage %q", usage)
	}
	return match, nil
}

func lifecycleRootKeyView(node map[string]any) RootKey {
	principal := node["principal"].(map[string]any)
	return RootKey{KeyID: node["key_id"].(string), Usage: node["usage"].(string),
		PublicKeyBase64URL: node["public_key_base64url"].(string),
		AuthorityDomain:    principal["authority_domain"].(string),
		PrincipalID:        principal["principal_id"].(string),
		PrincipalType:      principal["principal_type"].(string)}
}
