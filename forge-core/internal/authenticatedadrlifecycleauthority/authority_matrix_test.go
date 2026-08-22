//go:build unix && !aix && !solaris

package authenticatedadrlifecycleauthority

import (
	"bytes"
	"testing"

	approvalcontract "forgeos/forge-core/internal/authenticatedadrapprovalcontract"
)

func TestFixtureLifecycleAuthorityAndRootPinFailClosed(t *testing.T) {
	golden := loadJSONObject(t, "../../../docs/contracts/fixtures/authenticated-architecture-decision-lifecycle-v1.json")
	root := cloneObject(golden["lifecycle_trust_root"])
	trust := ExternalTrust{PinnedApprovalTrustRootSHA256: string(bytes.Repeat([]byte{'a'}, 64)),
		PinnedApprovalTrustEpoch: 1, PinnedLifecycleTrustRootSHA256: root["root_sha256"].(string),
		PinnedLifecycleTrustEpoch: 1, ObservedAtUnixMS: 1}
	_, _, err := validateLifecycleRoot(root, trust)
	assertLifecycleCode(t, err, codeFixtureAuthority)

	fixture := newAuthorityFixture(t)
	trust = fixture.lifecycleTrust()
	trust.PinnedLifecycleTrustRootSHA256 = string(bytes.Repeat([]byte{'0'}, 64))
	_, _, err = validateLifecycleRoot(fixture.lifecycleRoot, trust)
	assertLifecycleCode(t, err, codeTrustRootRejected)
}

func TestFixtureApprovalRootRejectedWithIndependentLifecycleRoot(t *testing.T) {
	fixture := newAuthorityFixture(t)
	golden := loadJSONObject(t, "../../../docs/contracts/fixtures/authenticated-architecture-decision-approval-v1.json")
	approvalRoot := cloneObject(golden["trust_root"])
	trust := fixture.lifecycleTrust()
	trust.PinnedApprovalTrustRootSHA256 = approvalRoot["root_sha256"].(string)
	trust.PinnedApprovalTrustEpoch = approvalRoot["trust_epoch"].(int64)
	_, err := decodeAuthorityMaterial(fixture.profile, canonicalForTest(t, approvalRoot),
		canonicalForTest(t, fixture.lifecycleRoot), trust)
	assertLifecycleCode(t, err, codeFixtureAuthority)
}

func TestLifecycleFixtureIdentityVariantsFailClosed(t *testing.T) {
	fixture := newAuthorityFixture(t)
	mutations := map[string]func(map[string]any){
		"exact domain": func(root map[string]any) {
			root["trust_domain"] = "forgeos.fixture"
			for _, raw := range root["keys"].([]any) {
				raw.(map[string]any)["principal"].(map[string]any)["authority_domain"] = "forgeos.fixture"
			}
		},
		"exact key id": func(root map[string]any) {
			lifecycleKeyForTest(root, requestUsage)["key_id"] = "fixture"
		},
		"renamed known public keys": func(root map[string]any) {
			golden := loadJSONObject(t, "../../../docs/contracts/fixtures/authenticated-architecture-decision-lifecycle-v1.json")
			replacement := renamedGoldenLifecycleRoot(cloneObject(golden["lifecycle_trust_root"]))
			for key := range root {
				delete(root, key)
			}
			for key, value := range replacement {
				root[key] = value
			}
		},
	}
	for name, mutate := range mutations {
		t.Run(name, func(t *testing.T) {
			root := cloneObject(fixture.lifecycleRoot)
			mutate(root)
			resealLifecycleRoot(t, root)
			trust := fixture.lifecycleTrust()
			trust.PinnedLifecycleTrustRootSHA256 = root["root_sha256"].(string)
			_, _, err := validateLifecycleRoot(root, trust)
			assertLifecycleCode(t, err, codeFixtureAuthority)
		})
	}
}

func renamedGoldenLifecycleRoot(root map[string]any) map[string]any {
	root["trust_domain"] = "forgeos.test.renamed-lifecycle-fixture"
	for _, raw := range root["keys"].([]any) {
		key := raw.(map[string]any)
		if key["usage"] == requestUsage {
			key["key_id"] = "renamed-lifecycle-request-key"
		} else {
			key["key_id"] = "renamed-lifecycle-state-key"
		}
		principal := key["principal"].(map[string]any)
		principal["authority_domain"] = root["trust_domain"]
		principal["principal_id"] = "renamed-" + principal["principal_type"].(string)
	}
	return root
}

func TestSignatureDomainsKeysAndIndependentPinsReject(t *testing.T) {
	fixture := newAuthorityFixture(t)
	authorization := fixture.approvalStored(t)
	request := loadRawObject(t, fixture.lifecycleInput(t, authorization).RequestJSON)
	digest := request["request_sha256"].(string)
	request["signature"].(map[string]any)["signature_base64url"] = signTestDigest(t,
		fixture.lifecycleRequestPrivate, acceptanceSignDomain, digest)
	_, err := TransitionAndStore(fixture.lifecycleConfig,
		EncodedTransitionInput{RequestJSON: canonicalForTest(t, request)}, authorization,
		fixture.lifecycleTrust())
	assertLifecycleCode(t, err, codeSignatureRejected)

	trust := fixture.lifecycleTrust()
	trust.PinnedLifecycleTrustRootSHA256 = trust.PinnedApprovalTrustRootSHA256
	_, err = TransitionAndStore(fixture.lifecycleConfig, fixture.lifecycleInput(t, authorization), authorization, trust)
	assertLifecycleCode(t, err, codeTrustRootRejected)
}

func TestApprovalAndLifecycleAuthorityFactsCannotBeReused(t *testing.T) {
	fixture := newAuthorityFixture(t)
	approval, err := approvalcontract.DecodeCanonicalTrustRoot(canonicalForTest(t, fixture.root))
	if err != nil {
		t.Fatal(err)
	}
	facts, err := approvalcontract.Facts(approval)
	if err != nil {
		t.Fatal(err)
	}
	mutations := authorityReuseMutations(facts.TrustDomain, facts.RootKeys[0])
	for name, mutate := range mutations {
		t.Run(name, func(t *testing.T) {
			root := cloneObject(fixture.lifecycleRoot)
			mutate(root)
			resealLifecycleRoot(t, root)
			trust := fixture.lifecycleTrust()
			trust.PinnedLifecycleTrustRootSHA256 = root["root_sha256"].(string)
			_, decodeErr := decodeAuthorityMaterial(fixture.profile,
				canonicalForTest(t, fixture.root), canonicalForTest(t, root), trust)
			assertLifecycleCode(t, decodeErr, codeTrustRootRejected)
		})
	}
}

func TestLifecycleRootMutationCannotRetainItsPinnedSHA(t *testing.T) {
	fixture := newAuthorityFixture(t)
	root := cloneObject(fixture.lifecycleRoot)
	root["trust_domain"] = "forgeos.test.lifecycle-mutated"
	_, _, err := validateLifecycleRoot(root, fixture.lifecycleTrust())
	assertLifecycleCode(t, err, codeTrustRootRejected)
}

func authorityReuseMutations(domain string,
	key approvalcontract.RootKey) map[string]func(map[string]any) {
	return map[string]func(map[string]any){
		"trust domain": func(root map[string]any) {
			root["trust_domain"] = domain
			for _, raw := range root["keys"].([]any) {
				raw.(map[string]any)["principal"].(map[string]any)["authority_domain"] = domain
			}
		},
		"key id": func(root map[string]any) {
			lifecycleKeyForTest(root, requestUsage)["key_id"] = key.KeyID
		},
		"public key": func(root map[string]any) {
			lifecycleKeyForTest(root, requestUsage)["public_key_base64url"] = key.PublicKeyBase64URL
		},
		"usage": func(root map[string]any) {
			lifecycleKeyForTest(root, requestUsage)["usage"] = key.Usage
		},
	}
}

func lifecycleKeyForTest(root map[string]any, usage string) map[string]any {
	for _, raw := range root["keys"].([]any) {
		key := raw.(map[string]any)
		if key["usage"] == usage {
			return key
		}
	}
	return nil
}

func resealLifecycleRoot(t *testing.T, root map[string]any) {
	t.Helper()
	root["root_sha256"] = ""
	sortNodes(root["keys"].([]any))
	digest, err := digestFor("root", root)
	if err != nil {
		t.Fatal(err)
	}
	root["root_sha256"] = digest
}

func TestCapacityTargetAndBootstrapExclusions(t *testing.T) {
	entries := make([]any, maxEntries)
	decisions := make([]any, maxDecisions)
	input := preparedInput{sequence: int64(maxEntries + 1), request: map[string]any{}}
	err := requireFreshPosition(input, &authenticatedState{}, entries, decisions)
	assertLifecycleCode(t, err, codeCapacityExhausted)

	targets := make([]any, maxTargets+1)
	for index := range targets {
		targets[index] = map[string]any{"adr_id": "ADR-9999"}
	}
	_, err = targetIDs(map[string]any{"supersession_targets": targets})
	if err == nil {
		t.Fatal("65 supersession targets passed")
	}
	for adrID := range excludedADRIDs {
		err = rejectProposal(adrID, string(bytes.Repeat([]byte{'a'}, 64)), nil)
		assertLifecycleCode(t, err, codeProposalExcluded)
	}
}
