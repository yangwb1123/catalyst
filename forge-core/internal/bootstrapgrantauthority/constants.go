package bootstrapgrantauthority

const (
	canonicalization        = "forgeos.canonical-json/v1"
	contractProfileID       = "bootstrap_planning_repo_read_only_v1"
	runtimeProfile          = "authenticated_bootstrap_repo_read_grant_issuance_v1"
	signatureProfile        = "forgeos.ed25519-domain-sha256/v1"
	effectVocabularySHA256  = "a45de832e43ccdbebcb22f183575039d451594bfbc9ec713105c657a6adda49f"
	knownFixtureRootSHA256  = "35b180615e7e4784e06d4c5618370904cd5b6824bc7e541bfeab62939891a27e"
	fixtureGrantPublicKey   = "_6pei1sR4U3wBG886tgROkh0V4HtRAfA3_MvDCH74gg"
	fixturePolicyPublicKey  = "IQg321jcqrX0ONYoqdDxnUS02yPv-ZbHYTqEkfSmmHk"
	fixtureRequestPublicKey = "l2I_Cm8HSlB-IHsGDPaTdinlassPI5qdqCBpuwZmZlw"

	profileAPI = "forgeos.ed25519-domain-sha256-profile/v1"
	rootAPI    = "forgeos.governance-trust-root/v1"
	policyAPI  = "forgeos.bootstrap-grant-policy/v1"
	requestAPI = "forgeos.bootstrap-grant-request/v1"
	receiptAPI = "forgeos.grant-issuance-receipt/v1"
	ledgerAPI  = "forgeos.grant-issuance-ledger/v1"
	resultAPI  = "forgeos.bootstrap-grant-issuance-result/v1"

	maxProfileBytes = 16 * 1024
	maxRootBytes    = 256 * 1024
	maxPolicyBytes  = 512 * 1024
	maxRequestBytes = 1024 * 1024
	maxGrantBytes   = 1024 * 1024
	maxReceiptBytes = 256 * 1024
	maxResultBytes  = 2 * 1024 * 1024
	maxLedgerBytes  = 16 * 1024 * 1024
	maxGoldenBytes  = 20 * 1024 * 1024
	maxTTLMillis    = int64(3_600_000)
	maxPolicyWindow = int64(86_400_000)
	maxRequestAge   = int64(300_000)
	maxOutputBytes  = int64(1_048_576)
	maxTimeout      = int64(300_000)
	maxLedgerItems  = 256
)

func isKnownFixturePublicKey(value string) bool {
	return value == fixtureGrantPublicKey || value == fixturePolicyPublicKey ||
		value == fixtureRequestPublicKey
}

var (
	profileDomain       = []byte("forgeos.ed25519-domain-sha256-profile.v1\x00")
	rootDomain          = []byte("forgeos.governance-trust-root.v1\x00")
	policyDomain        = []byte("forgeos.bootstrap-grant-policy.v1\x00")
	requestDomain       = []byte("forgeos.bootstrap-grant-request.v1\x00")
	grantEnvelopeDomain = []byte("forgeos.capability-grant.envelope.v1\x00")
	recordKeyDomain     = []byte("forgeos.grant-issuance-record-key.v1\x00")
	receiptDomain       = []byte("forgeos.grant-issuance-receipt.v1\x00")
	ledgerDomain        = []byte("forgeos.grant-issuance-ledger.v1\x00")

	policySignatureDomain  = []byte("forgeos.bootstrap-grant-policy.signature.v1\x00")
	requestSignatureDomain = []byte("forgeos.bootstrap-grant-request.signature.v1\x00")
	grantSignatureDomain   = []byte("forgeos.capability-grant.signature.v1\x00")
	receiptSignatureDomain = []byte("forgeos.grant-issuance-receipt.signature.v1\x00")
	ledgerSignatureDomain  = []byte("forgeos.grant-issuance-ledger.signature.v1\x00")
)
