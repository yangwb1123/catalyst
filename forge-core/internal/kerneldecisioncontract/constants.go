package kerneldecisioncontract

const (
	canonicalization    = "forgeos.canonical-json/v1"
	atomAPI             = "forgeos.aadm.cognitive-atom/v2"
	atomKind            = "CognitiveAtom"
	atomPrefix          = "cognitive-atom-"
	transactionAPI      = "forgeos.aadm.decision-transaction/v1"
	transactionKind     = "DecisionTransaction"
	transactionPrefix   = "decision-transaction-"
	closureAPI          = "forgeos.kernel-decision-reference-closure/v1"
	closureKind         = "KernelDecisionReferenceClosure"
	closurePrefix       = "kernel-decision-reference-closure-"
	maxAtomBytes        = 131072
	maxAtomSetBytes     = 1048576
	maxTransactionBytes = 1048576
	maxClosureBytes     = 20971520
	maxAtoms            = 256
	maxOptions          = 16
	maxAtomRefs         = 64
	maxIOItems          = 32
	maxProofs           = 32
	maxEvidenceKinds    = 16
	maxSelectorBytes    = 4096
	maxShortBytes       = 160
	maxStringBytes      = 16384
	maxTimeoutMS        = 86400000
)

var atomDomain = []byte("forgeos.aadm.cognitive-atom.v2\x00")
var transactionDomain = []byte("forgeos.aadm.decision-transaction.v1\x00")
var closureDomain = []byte("forgeos.kernel-decision-reference-closure.v1\x00")

const SuccessMarker = "STRUCTURALLY_VALID_KERNEL_DECISION_REFERENCE_CLOSURE_V1 (exact caller-supplied cognitive, transaction and operational reference relations only; declared authority and hardness are ineffective; all twenty-two attestations are false: no Approval, principal, Grant or binding authentication; no source resolution, authority, authorization, CAS, completion, content provenance, effect, event append, execution, hard guard, instruction, outcome, permission, persistence, transition, truth, usage measurement or verifier independence attestation)"
