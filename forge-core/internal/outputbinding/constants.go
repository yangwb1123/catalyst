// Package outputbinding defines ForgeOS's authority-neutral, local digest
// observations for accepted agent output. It does not authenticate a principal,
// evaluate policy, grant approval, or provide durable storage by itself.
package outputbinding

const (
	canonicalization   = "forgeos.canonical-json/v1"
	manifestAPI        = "forgeos.agent-output-artifact-manifest/v1"
	policyAPI          = "forgeos.local-runtime-policy-binding/v1"
	preflightAPI       = "forgeos.agent-output-preflight-binding/v1"
	receiptAPI         = "forgeos.agent-output-receipt/v1"
	receiptKind        = "AgentOutputObservation"
	localProfile       = "local_digest_v1"
	sourceStateProfile = "product-source-state-v1"

	manifestDomain  = "forgeos.agent-output-artifact-manifest.v1\x00"
	policyDomain    = "forgeos.local-runtime-policy-binding.v1\x00"
	preflightDomain = "forgeos.agent-output-preflight-binding.v1\x00"
	receiptDomain   = "forgeos.agent-output-receipt.v1\x00"

	maxIdentifierBytes = 256
	maxReferenceBytes  = 4096
	maxManifestItems   = 4096
	maxManifestBytes   = 20 << 20
	maxPolicyBytes     = 128 << 10
	maxPreflightBytes  = 128 << 10
	maxReceiptBytes    = 42 << 20
	maxArtifactBytes   = int64(1 << 40)
	maxOutputBytes     = int64(1 << 30)
	maxSequence        = int64(1 << 53)
)

// LocalDigestProfile is the opt-in workflow output-binding contract this
// package seals and validates.
const LocalDigestProfile = localProfile
