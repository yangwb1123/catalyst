package authenticatedadrlifecycleauthority

import (
	"fmt"
	"strings"

	approvalcontract "forgeos/forge-core/internal/authenticatedadrapprovalcontract"
)

const knownApprovalFixtureRootSHA256 = "e034d24aceb28b3087eb1c7132b77f444d5c42907719271602455d6e175cc790"
const knownLifecycleFixtureRootSHA256 = "cd7ced19dbd53eaa289851c03b3a7d78adacb58023ece8c5b4deaacacd915d07"

var knownApprovalFixturePublicKeys = map[string]bool{
	"AQEBAQEBAQEBAQEBAQEBAQEBAQEBAQEBAQEBAQEBAQE": true,
	"AgICAgICAgICAgICAgICAgICAgICAgICAgICAgICAgI": true,
	"AwMDAwMDAwMDAwMDAwMDAwMDAwMDAwMDAwMDAwMDAwM": true,
	"BAQEBAQEBAQEBAQEBAQEBAQEBAQEBAQEBAQEBAQEBAQ": true,
	"BQUFBQUFBQUFBQUFBQUFBQUFBQUFBQUFBQUFBQUFBQU": true,
	"BgYGBgYGBgYGBgYGBgYGBgYGBgYGBgYGBgYGBgYGBgY": true,
}

var knownLifecycleFixturePublicKeys = map[string]bool{
	"FRUVFRUVFRUVFRUVFRUVFRUVFRUVFRUVFRUVFRUVFRU": true,
	"FhYWFhYWFhYWFhYWFhYWFhYWFhYWFhYWFhYWFhYWFhY": true,
}

func rejectLifecycleFixtureFacts(root map[string]any, rootSHA string) error {
	domain, err := stringField(root, "trust_domain")
	if err != nil {
		return coded(codeTrustRootRejected, err)
	}
	if rootSHA == knownLifecycleFixtureRootSHA256 || fixtureAuthorityNamespace(domain) {
		return coded(codeFixtureAuthority, fmt.Errorf("fixture lifecycle root rejected"))
	}
	keys, err := arrayField(root, "keys")
	if err != nil {
		return coded(codeTrustRootRejected, err)
	}
	for _, raw := range keys {
		key, keyErr := objectValue(raw, "lifecycle root key")
		if keyErr != nil {
			return coded(codeTrustRootRejected, keyErr)
		}
		if keyErr = rejectLifecycleFixtureKey(key); keyErr != nil {
			return keyErr
		}
	}
	return nil
}

func rejectLifecycleFixtureKey(key map[string]any) error {
	keyID, keyErr := stringField(key, "key_id")
	publicKey, publicErr := stringField(key, "public_key_base64url")
	principal, principalErr := objectField(key, "principal")
	if keyErr != nil || publicErr != nil || principalErr != nil {
		return coded(codeTrustRootRejected, fmt.Errorf("lifecycle fixture identity is invalid"))
	}
	domain, domainErr := stringField(principal, "authority_domain")
	principalID, idErr := stringField(principal, "principal_id")
	if domainErr != nil || idErr != nil {
		return coded(codeTrustRootRejected, fmt.Errorf("lifecycle fixture principal is invalid"))
	}
	if knownLifecycleFixturePublicKeys[publicKey] || fixtureAuthorityIdentifier(keyID) ||
		fixtureAuthorityNamespace(domain) || fixtureAuthorityIdentifier(principalID) {
		return coded(codeFixtureAuthority, fmt.Errorf("fixture lifecycle key rejected"))
	}
	return nil
}

func rejectApprovalFixtureFacts(rootSHA, domain string,
	keys []approvalcontract.RootKey) error {
	if rootSHA == knownApprovalFixtureRootSHA256 || fixtureAuthorityNamespace(domain) {
		return coded(codeFixtureAuthority, fmt.Errorf("fixture approval root rejected"))
	}
	for _, key := range keys {
		if knownApprovalFixturePublicKeys[key.PublicKeyBase64URL] ||
			fixtureAuthorityIdentifier(key.KeyID) || fixtureAuthorityNamespace(key.AuthorityDomain) {
			return coded(codeFixtureAuthority, fmt.Errorf("fixture approval key rejected"))
		}
	}
	return nil
}

func fixtureAuthorityIdentifier(value string) bool {
	return value == "fixture" || strings.HasPrefix(value, "fixture-")
}

func fixtureAuthorityNamespace(value string) bool {
	return value == "forgeos.fixture" || strings.HasPrefix(value, "forgeos.fixture.")
}
