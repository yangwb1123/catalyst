package bootstraprepoexecutionauthority

import (
	"crypto/ed25519"
	"fmt"

	"forgeos/forge-core/internal/bootstrapgrantauthority"
)

var rootKeys = []string{"api_version", "canonicalization", "issuance_trust_epoch",
	"issuance_trust_root_sha256", "keys", "kind", "profile_id", "root_sha256",
	"signature_profile_sha256", "trust_domain", "trust_epoch"}
var rootKeyKeys = []string{"key_id", "principal", "public_key_base64url", "usage"}

type rootKey struct {
	id, publicKey string
	principal     map[string]any
}

// Trust is a validated execution signature profile and externally pinned root.
type Trust struct {
	profileHash, rootHash, domain, issuanceRootHash string
	epoch, issuanceEpoch                            int64
	keys                                            map[string]rootKey
}

// DecodePinnedTrustRoot authenticates root identity and issuance-root separation.
func DecodePinnedTrustRoot(data []byte, pinnedRootSHA256 string,
	issuance *bootstrapgrantauthority.Trust) (*Trust, error) {
	if issuance == nil {
		return nil, fmt.Errorf("issuance Trust is required")
	}
	issuanceBytes, err := issuance.ExecutionBindingJSON()
	if err != nil {
		return nil, err
	}
	issuanceBinding, err := decodeCanonical(issuanceBytes, maxRootBytes)
	if err != nil {
		return nil, err
	}
	return decodePinnedRoot(data, pinnedRootSHA256, issuanceBinding)
}

func decodePinnedRoot(data []byte, pin string, issuance map[string]any) (*Trust, error) {
	profileHash := frozenSignatureProfile()["profile_sha256"].(string)
	root, err := decodeCanonical(data, maxRootBytes)
	if err != nil {
		return nil, err
	}
	keys, err := validateTrustRoot(root, profileHash, issuance)
	if err != nil {
		return nil, err
	}
	rootHash := root["root_sha256"].(string)
	if !constantTimeTextEqual(rootHash, pin) {
		return nil, fmt.Errorf("execution TrustRoot does not match the external pin")
	}
	if containsKnownFixtureAuthority(rootHash, keys) {
		return nil, fmt.Errorf("known public fixture authority is forbidden at runtime")
	}
	epoch, _ := intValue(root, "trust_epoch")
	issuanceEpoch, _ := intValue(root, "issuance_trust_epoch")
	domain, _ := stringValue(root, "trust_domain")
	return &Trust{profileHash: profileHash, rootHash: rootHash, domain: domain,
		epoch: epoch, issuanceEpoch: issuanceEpoch,
		issuanceRootHash: root["issuance_trust_root_sha256"].(string), keys: keys}, nil
}

func frozenSignatureProfile() map[string]any {
	profile := map[string]any{"algorithm": "Ed25519", "api_version": profileAPI,
		"canonicalization": canonicalization, "digest_algorithm": "SHA-256",
		"kind":             "SignatureProfile",
		"message_preimage": "domain_separator_utf8_nul_then_raw_32_byte_sha256_digest",
		"profile_id":       signatureProfile, "profile_sha256": "",
		"public_key_encoding": "base64url_unpadded_32_bytes",
		"signature_encoding":  "base64url_unpadded_64_bytes"}
	digest, err := selfDigest(profileDomain, profile, "profile_sha256", maxProfileBytes,
		"SignatureProfile", false, "")
	if err != nil {
		panic(err)
	}
	profile["profile_sha256"] = digest
	return profile
}

func validateTrustRoot(root map[string]any, profileHash string,
	issuance map[string]any) (map[string]rootKey, error) {
	if err := requireKeys(root, rootKeys...); err != nil {
		return nil, fmt.Errorf("BootstrapRepoReadExecutionTrustRoot: %w", err)
	}
	if err := validateRootEnvelope(root, profileHash, issuance); err != nil {
		return nil, err
	}
	keys, err := validateRootKeySet(root, issuance)
	if err != nil {
		return nil, err
	}
	digest, err := selfDigest(rootDomain, root, "root_sha256", maxRootBytes,
		"BootstrapRepoReadExecutionTrustRoot", false, "")
	if err != nil || root["root_sha256"] != digest {
		return nil, fmt.Errorf("execution TrustRoot self digest does not match")
	}
	return keys, nil
}

func validateRootEnvelope(root map[string]any, profileHash string,
	issuance map[string]any) error {
	literals := map[string]string{"api_version": rootAPI, "canonicalization": canonicalization,
		"kind": "BootstrapRepoReadExecutionTrustRoot", "profile_id": profileID}
	for key, expected := range literals {
		if err := requireLiteral(root, key, expected); err != nil {
			return err
		}
	}
	if root["signature_profile_sha256"] != profileHash ||
		root["issuance_trust_root_sha256"] != issuance["trust_root_sha256"] ||
		root["issuance_trust_epoch"] != issuance["trust_epoch"] {
		return fmt.Errorf("execution root issuance or signature-profile binding is invalid")
	}
	if epoch, err := intValue(root, "trust_epoch"); err != nil || epoch < 1 {
		return fmt.Errorf("execution trust_epoch must be positive")
	}
	domain, err := stringValue(root, "trust_domain")
	if err != nil {
		return err
	}
	if err = validateText(domain, "trust_domain", 160); err != nil {
		return err
	}
	return validateHashField(root, "root_sha256", "root_sha256")
}

func validateRootKeySet(root map[string]any, issuance map[string]any) (map[string]rootKey, error) {
	values, err := arrayValue(root, "keys")
	if err != nil || len(values) != 3 {
		return nil, fmt.Errorf("execution TrustRoot must contain exactly three keys")
	}
	issuancePublics, err := issuancePublicKeys(issuance)
	if err != nil {
		return nil, err
	}
	usages := []string{"execution_policy_sign", "execution_receipt_sign", "execution_request_auth"}
	result := make(map[string]rootKey, 3)
	ids, principals, publics := map[string]bool{}, map[string]bool{}, map[string]bool{}
	for index, usage := range usages {
		key, keyErr := validateRootKey(values[index], usage)
		if keyErr != nil {
			return nil, keyErr
		}
		principalBytes, _ := canonicalJSON(key.principal)
		if ids[key.id] || principals[string(principalBytes)] || publics[key.publicKey] || issuancePublics[key.publicKey] {
			return nil, fmt.Errorf("execution and issuance key identities must be pairwise distinct")
		}
		ids[key.id], principals[string(principalBytes)], publics[key.publicKey] = true, true, true
		result[usage] = key
	}
	return result, nil
}

func validateRootKey(value any, usage string) (rootKey, error) {
	node, ok := value.(map[string]any)
	if !ok || requireKeys(node, rootKeyKeys...) != nil || node["usage"] != usage {
		return rootKey{}, fmt.Errorf("execution TrustRoot key shape or order is invalid")
	}
	id, err := stringValue(node, "key_id")
	if err != nil || validateText(id, "key_id", 160) != nil {
		return rootKey{}, fmt.Errorf("execution key_id is invalid")
	}
	principal, err := validatePrincipal(node["principal"], "root key principal")
	if err != nil {
		return rootKey{}, err
	}
	expectedType := "service"
	if usage == "execution_request_auth" {
		expectedType = "agent"
	}
	if principal["principal_type"] != expectedType {
		return rootKey{}, fmt.Errorf("%s principal must be %s", usage, expectedType)
	}
	publicKey, err := stringValue(node, "public_key_base64url")
	if err != nil {
		return rootKey{}, err
	}
	if _, err = decodeBase64URL(publicKey, "public_key_base64url", ed25519.PublicKeySize); err != nil {
		return rootKey{}, err
	}
	return rootKey{id: id, publicKey: publicKey, principal: principal}, nil
}

func issuancePublicKeys(binding map[string]any) (map[string]bool, error) {
	keys, err := arrayValue(binding, "keys")
	if err != nil || len(keys) != 3 {
		return nil, fmt.Errorf("issuance key projection is invalid")
	}
	publics := make(map[string]bool, 3)
	for _, value := range keys {
		key, ok := value.(map[string]any)
		publicKey, keyErr := stringValue(key, "public_key_base64url")
		if !ok || keyErr != nil {
			return nil, fmt.Errorf("issuance key projection is invalid")
		}
		publics[publicKey] = true
	}
	return publics, nil
}

func containsKnownFixtureAuthority(rootHash string, keys map[string]rootKey) bool {
	if knownFixtureRootSHA256 != "" && rootHash == knownFixtureRootSHA256 {
		return true
	}
	for _, key := range keys {
		if isKnownFixturePublicKey(key.publicKey) {
			return true
		}
	}
	return false
}
