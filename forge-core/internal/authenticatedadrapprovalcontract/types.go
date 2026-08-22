package authenticatedadrapprovalcontract

// Bundle is an opaque, validated ADR-0079 candidate bundle.
type Bundle struct {
	document map[string]any
	root     *TrustRoot
}

// TrustRoot is an opaque, structurally valid caller-supplied trust root.
// Its validity does not establish an external trust pin.
type TrustRoot struct {
	document map[string]any
}

// AuthorizationInput is a validated exact proposal, policy, complete
// revocation chain, and request tuple. It carries no authentication result.
type AuthorizationInput struct {
	proposal  []byte
	policy    map[string]any
	snapshots []any
	request   map[string]any
	metadata  proposalMetadata
	root      *TrustRoot
}

// EncodedAuthorizationInput contains the caller-supplied exact input bytes.
type EncodedAuthorizationInput struct {
	ProposalDocument    []byte
	Policy              []byte
	RevocationSnapshots [][]byte
	Request             []byte
}

// Receipt is an opaque structurally valid authorization-shaped receipt.
type Receipt struct {
	document map[string]any
	root     *TrustRoot
	input    *AuthorizationInput
}

// Ledger is an opaque structurally valid complete caller-supplied ledger.
type Ledger struct {
	document map[string]any
	root     *TrustRoot
}

// RootKey is a detached root-key view.
type RootKey struct {
	KeyID              string
	AuthorityDomain    string
	PrincipalID        string
	PrincipalType      string
	PublicKeyBase64URL string
	Usage              string
}

type proposalMetadata struct {
	ADRID            string
	ApproverRefs     []string
	BodySHA256       string
	DocumentName     string
	ExpiresAtUnixMS  *int64
	OwnerRefs        []string
	ProposedAtUnixMS int64
	SelfSHA256       string
	Status           string
}

type ledgerPosition struct {
	ClockHighWaterUnixMS        int64
	LastReceiptSHA256           string
	LedgerSHA256                string
	NextSequence                int64
	RevocationHighWaterSHA256   string
	RevocationHighWaterSequence int64
}

// SignatureCheck is a proof-shaped verification input. Returning one does not
// verify its signature, key, currentness, or external root pin.
type SignatureCheck struct {
	Artifact       string
	ArtifactSHA256 string
	Domain         string
	Key            RootKey
	Message        []byte
	Signature      []byte
}

// ReceiptDraft is an opaque deterministic receipt preimage awaiting an
// externally produced proof-shaped signature.
type ReceiptDraft struct {
	document map[string]any
	input    *AuthorizationInput
}

// LedgerDraft is an opaque deterministic ledger preimage awaiting an
// externally produced proof-shaped signature.
type LedgerDraft struct {
	document map[string]any
	root     *TrustRoot
}

type facts struct {
	Kind                         string
	TrustDomain                  string
	TrustRootSHA256              string
	TrustEpoch                   int64
	RootKeys                     []RootKey
	ADRID                        string
	ProposalBindingSHA256        string
	IdempotencyKey               string
	RecordKeySHA256              string
	ExpectedNextSequence         int64
	ExpectedLedgerSHA256         *string
	RequestSHA256                string
	RequestedAtUnixMS            int64
	RequestExpiresAtUnixMS       int64
	RevocationSequence           int64
	RevocationSHA256             string
	RevocationEffectiveAtUnixMS  int64
	RevocationExpiresAtUnixMS    int64
	AuthorizationDecision        string
	AuthorizationExpiresAtUnixMS int64
	AuthorizationReasonCodes     []string
	QualifyingApprovalIDs        []string
	ReceiptEvaluatedAtUnixMS     int64
	ReceiptID                    string
	ReceiptLedgerSequence        int64
	ReceiptPriorSHA256           *string
	ReceiptSHA256                string
	DeliveryDisposition          string
	ClockHighWaterUnixMS         int64
	LedgerSHA256                 string
	RevocationHighWaterSequence  int64
	RevocationHighWaterSHA256    string
	ReplayRecords                []replayFact
}

type replayFact struct {
	AuthorizationDecision string
	ProposalBindingSHA256 string
	ReceiptSHA256         string
	RecordKeySHA256       string
	RequestSHA256         string
	Sequence              int64
}
