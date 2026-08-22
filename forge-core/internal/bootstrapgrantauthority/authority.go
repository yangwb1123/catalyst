package bootstrapgrantauthority

import (
	"crypto/ed25519"
	"fmt"
)

var profileKeys = []string{
	"algorithm", "api_version", "canonicalization", "digest_algorithm", "kind",
	"message_preimage", "profile_id", "profile_sha256", "public_key_encoding",
	"signature_encoding",
}

var rootKeys = []string{
	"api_version", "canonicalization", "keys", "kind", "profile_id", "root_sha256",
	"signature_profile_sha256", "trust_domain", "trust_epoch",
}

var rootKeyKeys = []string{"key_id", "principal", "public_key_base64url", "usage"}

type rootKey struct {
	id        string
	usage     string
	publicKey string
	principal map[string]any
}

// Trust is a validated frozen signature profile and externally pinned root.
type Trust struct {
	profile     map[string]any
	root        map[string]any
	profileHash string
	rootHash    string
	epoch       int64
	domain      string
	keys        map[string]rootKey
}

func frozenSignatureProfile() map[string]any {
	profile := map[string]any{
		"algorithm": "Ed25519", "api_version": profileAPI,
		"canonicalization": canonicalization, "digest_algorithm": "SHA-256",
		"kind":             "SignatureProfile",
		"message_preimage": "domain_separator_utf8_nul_then_raw_32_byte_sha256_digest",
		"profile_id":       signatureProfile, "profile_sha256": "",
		"public_key_encoding": "base64url_unpadded_32_bytes",
		"signature_encoding":  "base64url_unpadded_64_bytes",
	}
	digest, err := selfDigest(profileDomain, profile, "profile_sha256", maxProfileBytes,
		"SignatureProfile", false)
	if err != nil {
		panic(err)
	}
	profile["profile_sha256"] = digest
	return profile
}

// DecodePinnedTrustRoot requires strict bytes and the operator-supplied root digest.
func DecodePinnedTrustRoot(data []byte, pinnedRootSHA256 string) (*Trust, error) {
	profile := frozenSignatureProfile()
	profileHash := profile["profile_sha256"].(string)
	root, err := decodeCanonical(data, maxRootBytes)
	if err != nil {
		return nil, err
	}
	keys, err := validateTrustRoot(root, profileHash)
	if err != nil {
		return nil, err
	}
	rootHash := root["root_sha256"].(string)
	if !constantTimeTextEqual(rootHash, pinnedRootSHA256) {
		return nil, fmt.Errorf("GovernanceTrustRoot does not match the external pin")
	}
	if containsKnownFixtureAuthority(rootHash, keys) {
		return nil, fmt.Errorf("known public fixture authority is forbidden at runtime")
	}
	epoch, _ := intValue(root, "trust_epoch")
	domain, _ := stringValue(root, "trust_domain")
	return &Trust{profile: profile, root: root, profileHash: profileHash,
		rootHash: rootHash, epoch: epoch, domain: domain, keys: keys}, nil
}

func containsKnownFixtureAuthority(rootHash string, keys map[string]rootKey) bool {
	if rootHash == knownFixtureRootSHA256 {
		return true
	}
	for _, key := range keys {
		if isKnownFixturePublicKey(key.publicKey) {
			return true
		}
	}
	return false
}

func validateTrustRoot(root map[string]any, profileHash string) (map[string]rootKey, error) {
	if err := requireKeys(root, rootKeys...); err != nil {
		return nil, fmt.Errorf("GovernanceTrustRoot: %w", err)
	}
	if err := validateRootEnvelope(root, profileHash); err != nil {
		return nil, err
	}
	keys, err := validateRootKeySet(root)
	if err != nil {
		return nil, err
	}
	computed, err := selfDigest(rootDomain, root, "root_sha256", maxRootBytes,
		"GovernanceTrustRoot", false)
	if err != nil || root["root_sha256"] != computed {
		return nil, fmt.Errorf("GovernanceTrustRoot self digest does not match")
	}
	return keys, nil
}

func validateRootEnvelope(root map[string]any, profileHash string) error {
	literals := map[string]string{"api_version": rootAPI, "canonicalization": canonicalization,
		"kind": "GovernanceTrustRoot", "profile_id": contractProfileID}
	for key, expected := range literals {
		if err := requireLiteral(root, key, expected); err != nil {
			return err
		}
	}
	if err := validateTextField(root, "trust_domain", 160); err != nil {
		return err
	}
	if epoch, err := intValue(root, "trust_epoch"); err != nil || epoch < 1 {
		return fmt.Errorf("trust_epoch must be a positive signed int64")
	}
	if root["signature_profile_sha256"] != profileHash {
		return fmt.Errorf("GovernanceTrustRoot signature profile digest drifted")
	}
	if hash, err := stringValue(root, "root_sha256"); err != nil || validateHash(hash, "root_sha256") != nil {
		return fmt.Errorf("GovernanceTrustRoot root_sha256 is invalid")
	}
	return nil
}

func validateRootKeySet(root map[string]any) (map[string]rootKey, error) {
	values, err := arrayValue(root, "keys")
	if err != nil || len(values) != 3 {
		return nil, fmt.Errorf("GovernanceTrustRoot must contain exactly three keys")
	}
	usages := []string{"grant_issue", "policy_sign", "request_auth"}
	result := make(map[string]rootKey, 3)
	ids, principals, publics := map[string]bool{}, map[string]bool{}, map[string]bool{}
	for index, usage := range usages {
		key, keyErr := validateRootKey(values[index], usage)
		if keyErr != nil {
			return nil, keyErr
		}
		principalBytes, _ := canonicalJSON(key.principal)
		if ids[key.id] || principals[string(principalBytes)] || publics[key.publicKey] {
			return nil, fmt.Errorf("root key IDs, principals, and public keys must be pairwise distinct")
		}
		ids[key.id], principals[string(principalBytes)], publics[key.publicKey] = true, true, true
		result[usage] = key
	}
	return result, nil
}

func validateRootKey(value any, usage string) (rootKey, error) {
	node, ok := value.(map[string]any)
	if !ok || requireKeys(node, rootKeyKeys...) != nil {
		return rootKey{}, fmt.Errorf("GovernanceTrustRoot key fields are invalid")
	}
	id, err := stringValue(node, "key_id")
	if err != nil || validateText(id, "key_id", 160) != nil || node["usage"] != usage {
		return rootKey{}, fmt.Errorf("GovernanceTrustRoot key identity or usage is invalid")
	}
	principal, err := validatePrincipalNode(node["principal"], "root key principal")
	if err != nil {
		return rootKey{}, err
	}
	expectedType := "service"
	if usage == "request_auth" {
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
	return rootKey{id: id, usage: usage, publicKey: publicKey, principal: principal}, nil
}

func validateTextField(node map[string]any, key string, maximum int) error {
	value, err := stringValue(node, key)
	if err != nil {
		return err
	}
	return validateText(value, key, maximum)
}
