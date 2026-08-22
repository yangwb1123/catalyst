package bootstraprepoexecutionauthority

const (
	canonicalization = "forgeos.canonical-json/v1"
	profileID        = "authenticated_bootstrap_repo_read_execution_v1"
	signatureProfile = "forgeos.ed25519-domain-sha256/v1"

	profileAPI    = "forgeos.ed25519-domain-sha256-profile/v1"
	rootAPI       = "forgeos.bootstrap-repo-read-execution-trust-root/v1"
	manifestAPI   = "forgeos.repo-read-expected-manifest/v1"
	policyAPI     = "forgeos.bootstrap-repo-read-execution-policy/v1"
	invocationAPI = "forgeos.bootstrap-repo-read-invocation/v1"
	receiptAPI    = "forgeos.bootstrap-repo-read-usage-receipt/v1"
	resultAPI     = "forgeos.bootstrap-repo-read-execution-result/v1"
	metadataAPI   = "forgeos.bootstrap-repo-read-result-metadata/v1"
	ledgerAPI     = "forgeos.bootstrap-repo-read-usage-ledger/v1"
	deliveryAPI   = "forgeos.bootstrap-repo-read-execution-delivery/v1"

	maxProfileBytes          = 16 * 1024
	maxRootBytes             = 256 * 1024
	maxManifestBytes         = 256 * 1024
	maxPolicyBytes           = 512 * 1024
	maxInvocationBytes       = 512 * 1024
	maxReceiptBytes          = 256 * 1024
	maxResultBytes           = 2 * 1024 * 1024
	maxMetadataBytes         = 256 * 1024
	maxDeliveryBytes         = 3 * 1024 * 1024
	maxLedgerBytes           = 16 * 1024 * 1024
	maxContentBytes          = int64(1024 * 1024)
	maxLedgerItems           = 256
	maxFreshnessMillis       = int64(300_000)
	reservationOverheadBytes = 4096
	orphanOverheadBytes      = 1024

	knownFixtureRootSHA256  = "ecdb7c3000c34ca55b05fe550562fe3fb17c8f616c4dd3676e477690a691b1e1"
	fixturePolicyPublicKey  = "CJrqk7P5yoNWX2Ix9yKqfadcoXTNNwux3v8GKbpfW9I"
	fixtureReceiptPublicKey = "FacOWKFvLiH1ImZ87bISTGC7hoMnqiOCo08nFA9PDVE"
	fixtureRequestPublicKey = "4YsrIsanPsYyfRQbwiHqgGdssFATDONdB18naPuaVm4"
)

var (
	profileDomain             = []byte("forgeos.ed25519-domain-sha256-profile.v1\x00")
	rootDomain                = []byte("forgeos.bootstrap-repo-read-execution-trust-root.v1\x00")
	manifestDomain            = []byte("forgeos.repo-read-expected-manifest.v1\x00")
	policyDomain              = []byte("forgeos.bootstrap-repo-read-execution-policy.v1\x00")
	policySignatureDomain     = []byte("forgeos.bootstrap-repo-read-execution-policy.signature.v1\x00")
	actionDomain              = []byte("forgeos.capability-requested-action.v1\x00")
	idempotencyDomain         = []byte("forgeos.bootstrap-repo-read-idempotency-record-key.v1\x00")
	invocationDomain          = []byte("forgeos.bootstrap-repo-read-invocation.v1\x00")
	invocationSignatureDomain = []byte("forgeos.bootstrap-repo-read-invocation.signature.v1\x00")
	resultDomain              = []byte("forgeos.bootstrap-repo-read-execution-result.v1\x00")
	metadataDomain            = []byte("forgeos.bootstrap-repo-read-result-metadata.v1\x00")
	receiptDomain             = []byte("forgeos.bootstrap-repo-read-usage-receipt.v1\x00")
	receiptSignatureDomain    = []byte("forgeos.bootstrap-repo-read-usage-receipt.signature.v1\x00")
	ledgerDomain              = []byte("forgeos.bootstrap-repo-read-usage-ledger.v1\x00")
	ledgerSignatureDomain     = []byte("forgeos.bootstrap-repo-read-usage-ledger.signature.v1\x00")
)

func isKnownFixturePublicKey(value string) bool {
	if value == "" {
		return false
	}
	return value == fixturePolicyPublicKey || value == fixtureReceiptPublicKey ||
		value == fixtureRequestPublicKey
}
