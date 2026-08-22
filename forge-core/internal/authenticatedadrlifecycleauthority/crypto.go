package authenticatedadrlifecycleauthority

import (
	"crypto/ed25519"
	"crypto/subtle"
	"encoding/base64"
	"fmt"

	approvalcontract "forgeos/forge-core/internal/authenticatedadrapprovalcontract"
	lifecyclecontract "forgeos/forge-core/internal/authenticatedadrlifecyclecontract"
)

type lifecycleKey struct {
	KeyID              string
	PublicKeyBase64URL string
	Usage              string
}

type authorityMaterial struct {
	profileRaw   []byte
	approvalRaw  []byte
	lifecycleRaw []byte
	profile      map[string]any
	approvalNode map[string]any
	approval     *approvalcontract.TrustRoot
	lifecycle    map[string]any
	requestKey   lifecycleKey
	stateKey     lifecycleKey
}

func decodeAuthorityMaterial(profileRaw, approvalRaw, lifecycleRaw []byte,
	trust ExternalTrust) (authorityMaterial, error) {
	var result authorityMaterial
	profileValue, err := parseCanonicalJSON(profileRaw, int(maxProfile), "signature profile")
	if err != nil {
		return result, coded(codeTrustRootRejected, err)
	}
	profile, err := objectValue(profileValue, "signature profile")
	if err != nil {
		return result, coded(codeTrustRootRejected, err)
	}
	profileDigest, err := digestFor("profile", profile)
	if err != nil || profileDigest != profileSHA256 || profile["profile_sha256"] != profileSHA256 {
		return result, coded(codeTrustRootRejected, fmt.Errorf("signature profile pin differs"))
	}
	approval, err := approvalcontract.DecodeCanonicalTrustRoot(approvalRaw)
	if err != nil {
		return result, coded(codeTrustRootRejected, err)
	}
	approvalFacts, err := approvalcontract.Facts(approval)
	if err != nil || !constantTimeEqual(approvalFacts.TrustRootSHA256,
		trust.PinnedApprovalTrustRootSHA256) || approvalFacts.TrustEpoch != trust.PinnedApprovalTrustEpoch {
		return result, coded(codeTrustRootRejected, fmt.Errorf("approval root differs from external pin"))
	}
	if err = rejectApprovalFixtureFacts(approvalFacts.TrustRootSHA256,
		approvalFacts.TrustDomain, approvalFacts.RootKeys); err != nil {
		return result, err
	}
	approvalNode, err := parseAuthorityObject(approvalRaw, "approval trust root")
	if err != nil {
		return result, coded(codeTrustRootRejected, err)
	}
	lifecycle, err := parseAuthorityObject(lifecycleRaw, "lifecycle trust root")
	if err != nil {
		return result, coded(codeTrustRootRejected, err)
	}
	requestKey, stateKey, err := validateLifecycleRoot(lifecycle, trust)
	if err != nil {
		return result, err
	}
	if err = validateAuthorityIndependence(lifecycle, approvalFacts.TrustDomain,
		approvalFacts.RootKeys); err != nil {
		return result, coded(codeTrustRootRejected, err)
	}
	result = authorityMaterial{profileRaw: cloneBytes(profileRaw), approvalRaw: cloneBytes(approvalRaw),
		lifecycleRaw: cloneBytes(lifecycleRaw), profile: profile, approvalNode: approvalNode,
		approval: approval, lifecycle: lifecycle, requestKey: requestKey, stateKey: stateKey}
	return result, nil
}

func parseAuthorityObject(raw []byte, label string) (map[string]any, error) {
	value, err := parseCanonicalJSON(raw, int(maxRoot), label)
	if err != nil {
		return nil, err
	}
	return objectValue(value, label)
}

func validateLifecycleRoot(root map[string]any,
	trust ExternalTrust) (lifecycleKey, lifecycleKey, error) {
	var request, state lifecycleKey
	digest, err := digestFor("root", root)
	if err != nil || root["root_sha256"] != digest ||
		!constantTimeEqual(digest, trust.PinnedLifecycleTrustRootSHA256) ||
		root["trust_epoch"] != trust.PinnedLifecycleTrustEpoch {
		return request, state, coded(codeTrustRootRejected, fmt.Errorf("lifecycle root differs from external pin"))
	}
	if err = rejectLifecycleFixtureFacts(root, digest); err != nil {
		return request, state, err
	}
	keys, err := arrayField(root, "keys")
	if err != nil || len(keys) != 2 {
		return request, state, coded(codeTrustRootRejected, fmt.Errorf("lifecycle root requires two keys"))
	}
	for _, raw := range keys {
		keyNode, keyErr := objectValue(raw, "lifecycle root key")
		if keyErr != nil {
			return request, state, coded(codeTrustRootRejected, keyErr)
		}
		key, keyErr := parseLifecycleKey(keyNode)
		if keyErr != nil {
			return request, state, coded(codeTrustRootRejected, keyErr)
		}
		switch key.Usage {
		case requestUsage:
			request = key
		case stateUsage:
			state = key
		default:
			return request, state, coded(codeTrustRootRejected, fmt.Errorf("unknown lifecycle key usage"))
		}
	}
	if request.KeyID == "" || state.KeyID == "" || request.KeyID == state.KeyID ||
		request.PublicKeyBase64URL == state.PublicKeyBase64URL {
		return request, state, coded(codeTrustRootRejected, fmt.Errorf("lifecycle keys are absent or reused"))
	}
	return request, state, nil
}

type rootIdentities struct {
	domains    map[string]bool
	keyIDs     map[string]bool
	publicKeys map[string]bool
	principals map[string]bool
	usages     map[string]bool
}

func validateAuthorityIndependence(lifecycle map[string]any, approvalDomain string,
	approvalKeys []approvalcontract.RootKey) error {
	approval := approvalRootIdentities(approvalDomain, approvalKeys)
	domain, err := stringField(lifecycle, "trust_domain")
	if err != nil || approval.domains[domain] {
		return fmt.Errorf("approval and lifecycle authority domains must differ")
	}
	keys, err := arrayField(lifecycle, "keys")
	if err != nil {
		return err
	}
	for _, raw := range keys {
		key, keyErr := objectValue(raw, "lifecycle root key")
		if keyErr != nil {
			return keyErr
		}
		if keyErr = rejectApprovalIdentityReuse(key, approval); keyErr != nil {
			return keyErr
		}
	}
	return nil
}

func approvalRootIdentities(domain string, keys []approvalcontract.RootKey) rootIdentities {
	result := rootIdentities{domains: map[string]bool{domain: true}, keyIDs: map[string]bool{},
		publicKeys: map[string]bool{}, principals: map[string]bool{}, usages: map[string]bool{}}
	for _, key := range keys {
		result.domains[key.AuthorityDomain] = true
		result.keyIDs[key.KeyID] = true
		result.publicKeys[key.PublicKeyBase64URL] = true
		result.usages[key.Usage] = true
		result.principals[rootPrincipalIdentity(key.AuthorityDomain,
			key.PrincipalID, key.PrincipalType)] = true
	}
	return result
}

func rejectApprovalIdentityReuse(key map[string]any, approval rootIdentities) error {
	keyID, keyErr := stringField(key, "key_id")
	publicKey, publicErr := stringField(key, "public_key_base64url")
	usage, usageErr := stringField(key, "usage")
	principal, principalErr := objectField(key, "principal")
	if keyErr != nil || publicErr != nil || usageErr != nil || principalErr != nil {
		return fmt.Errorf("lifecycle root key identity is invalid")
	}
	domain, domainErr := stringField(principal, "authority_domain")
	id, idErr := stringField(principal, "principal_id")
	kind, kindErr := stringField(principal, "principal_type")
	if domainErr != nil || idErr != nil || kindErr != nil {
		return fmt.Errorf("lifecycle root principal identity is invalid")
	}
	if approval.domains[domain] || approval.keyIDs[keyID] ||
		approval.publicKeys[publicKey] || approval.usages[usage] ||
		approval.principals[rootPrincipalIdentity(domain, id, kind)] {
		return fmt.Errorf("approval and lifecycle authority facts must be disjoint")
	}
	return nil
}

func rootPrincipalIdentity(domain, id, kind string) string {
	return domain + "\x00" + id + "\x00" + kind
}

func parseLifecycleKey(node map[string]any) (lifecycleKey, error) {
	var result lifecycleKey
	if err := requireFields(node, "lifecycle root key", "key_id", "principal", "public_key_base64url", "usage"); err != nil {
		return result, err
	}
	var err error
	result.KeyID, err = stringField(node, "key_id")
	if err != nil {
		return result, err
	}
	result.PublicKeyBase64URL, err = stringField(node, "public_key_base64url")
	if err != nil {
		return result, err
	}
	result.Usage, err = stringField(node, "usage")
	if err != nil {
		return result, err
	}
	if _, err = decodeFixedBase64(result.PublicKeyBase64URL, ed25519.PublicKeySize); err != nil {
		return result, err
	}
	return result, nil
}

func verifyBundleSignatures(bundle *lifecyclecontract.Bundle) error {
	checks, err := lifecyclecontract.SignatureChecks(bundle)
	if err != nil {
		return err
	}
	for _, check := range checks {
		publicKey, keyErr := decodeFixedBase64(check.Key.PublicKeyBase64URL, ed25519.PublicKeySize)
		if keyErr != nil || len(check.Signature) != ed25519.SignatureSize ||
			!ed25519.Verify(ed25519.PublicKey(publicKey), check.Message, check.Signature) {
			return fmt.Errorf("%s signature rejected", check.Artifact)
		}
	}
	return nil
}

func verifyRequestSignature(request map[string]any, key lifecycleKey) error {
	signature, err := objectField(request, "signature")
	if err != nil {
		return err
	}
	keyID, err := stringField(signature, "key_id")
	if err != nil || keyID != key.KeyID {
		return fmt.Errorf("request uses wrong key")
	}
	digest, err := stringField(request, "request_sha256")
	if err != nil {
		return err
	}
	proofText, err := stringField(signature, "signature_base64url")
	if err != nil {
		return err
	}
	proof, err := decodeFixedBase64(proofText, ed25519.SignatureSize)
	if err != nil {
		return err
	}
	publicKey, err := decodeFixedBase64(key.PublicKeyBase64URL, ed25519.PublicKeySize)
	if err != nil {
		return err
	}
	message, err := signatureMessage(requestSignDomain, digest)
	if err != nil {
		return err
	}
	if !ed25519.Verify(ed25519.PublicKey(publicKey), message, proof) {
		return fmt.Errorf("request signature rejected")
	}
	return nil
}

func decodeFixedBase64(value string, size int) ([]byte, error) {
	decoded, err := base64.RawURLEncoding.DecodeString(value)
	if err != nil || len(decoded) != size || base64.RawURLEncoding.EncodeToString(decoded) != value {
		return nil, fmt.Errorf("base64url value does not encode exactly %d bytes", size)
	}
	return decoded, nil
}

func constantTimeEqual(left, right string) bool {
	if len(left) != len(right) {
		return false
	}
	return subtle.ConstantTimeCompare([]byte(left), []byte(right)) == 1
}
