package authenticatedadrlifecyclecontract

import approvalcontract "forgeos/forge-core/internal/authenticatedadrapprovalcontract"

// Bundle is an opaque validated ADR-0082 structural candidate.
type Bundle struct {
	document      map[string]any
	approvalRoot  *approvalcontract.TrustRoot
	lifecycleRoot map[string]any
	profileHash   string
}

// RootKey is a detached proof-shaped lifecycle root-key view.
type RootKey struct {
	KeyID              string
	AuthorityDomain    string
	PrincipalID        string
	PrincipalType      string
	PublicKeyBase64URL string
	Usage              string
}

// DecisionFact is a detached structural materialized-view row.
type DecisionFact struct {
	ADRID                 string
	AcceptanceID          string
	AcceptanceSHA256      string
	AcceptedAtUnixMS      int64
	DocumentName          string
	ExpiresAtUnixMS       *int64
	ProposalBindingSHA256 string
	SourcePhysicalSHA256  string
	Status                string
	SupersededBy          []string
	Supersedes            []string
}

// EntryFact is a detached lifecycle/approval sequence observation.
type EntryFact struct {
	Sequence                int64
	ApprovalReceiptSequence int64
	ADRID                   string
	EntrySHA256             string
	RequestSHA256           string
	AcceptanceSHA256        string
	AcceptedAtUnixMS        int64
	TargetADRs              []string
}

// Facts is a detached structural projection. It is not currentness or authority.
type Facts struct {
	ApprovalTrustRootSHA256  string
	ApprovalTrustEpoch       int64
	LifecycleTrustDomain     string
	LifecycleTrustRootSHA256 string
	LifecycleTrustEpoch      int64
	LifecycleRootKeys        []RootKey
	LedgerSHA256             string
	LastSequence             int64
	CurrentHeadSetSHA256     string
	HeadADRIDs               []string
	Entries                  []EntryFact
	Decisions                []DecisionFact
	ResultDisposition        string
	ResultSequence           int64
	StateSHA256              string
}

// SignatureCheck is a detached proof-shaped signature input. It verifies no
// signature, external root pin, currentness, or authority.
type SignatureCheck struct {
	Artifact       string
	ArtifactSHA256 string
	Domain         string
	Key            RootKey
	Message        []byte
	Signature      []byte
}

type proposalMetadata struct {
	ADRID            string
	BodySHA256       string
	DocumentName     string
	ExpiresAtUnixMS  *int64
	PhysicalSHA256   string
	ProposedAtUnixMS int64
	SelfSHA256       string
	Supersedes       []string
}

type validationContext struct {
	profileHash   string
	approvalRoot  *approvalcontract.TrustRoot
	lifecycleRoot map[string]any
	rebuilt       map[string]map[string]any
}
