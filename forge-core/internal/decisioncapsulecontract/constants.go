package decisioncapsulecontract

const (
	canonicalization = "forgeos.canonical-json/v1"

	manifestAPI    = "forgeos.aadm.structural-replay-manifest/v1"
	manifestKind   = "StructuralReplayManifest"
	manifestPrefix = "structural-replay-manifest-"
	manifestMode   = "structural_validate_reseal_compare_only"

	capsuleAPI    = "forgeos.aadm.decision-capsule/v1"
	capsuleKind   = "DecisionCapsule"
	capsulePrefix = "decision-capsule-"
	capsuleMode   = "structural_replay_manifest_only"

	branchAPI        = "forgeos.aadm.evaluation-branch/v1"
	branchKind       = "EvaluationBranch"
	branchPrefix     = "evaluation-branch-"
	branchMode       = "structural_validate_reseal_compare_only"
	comparisonResult = "EXACT_STRUCTURAL_REFERENCE_MATCH_ONLY"

	closureAPI    = "forgeos.aadm.structural-replay-closure/v1"
	closureKind   = "StructuralReplayClosure"
	closurePrefix = "structural-replay-closure-"

	decisionClosurePrefix     = "kernel-decision-reference-closure-"
	decisionTransactionPrefix = "decision-transaction-"
	operationalClosurePrefix  = "kernel-operational-reference-closure-"
	atomPrefix                = "cognitive-atom-"
	artifactReceiptPrefix     = "artifact-receipt-"
	invocationPrefix          = "capability-invocation-"
	eventPrefix               = "interaction-event-"
	executionReceiptPrefix    = "execution-receipt-"

	maxManifestBytes        = 4 * 1024 * 1024
	maxCapsuleBytes         = 26 * 1024 * 1024
	maxBranchBytes          = 64 * 1024
	maxClosureBytes         = 28 * 1024 * 1024
	maxDecisionClosureBytes = 20 * 1024 * 1024

	maxDepth                = 16
	maxObjectFields         = 64
	maxArrayItems           = 256
	maxStringBytes          = 16 * 1024
	maxShortBytes           = 160
	maxReferenceBytes       = 4096
	maxAtoms                = 256
	maxArtifacts            = 256
	maxArtifactReceipts     = 64
	maxInvocations          = 64
	maxEvents               = 256
	maxExecutionReceipts    = 64
	maxReflectionReportRefs = 32
)

var (
	manifestDomain        = []byte("forgeos.aadm.structural-replay-manifest.v1\x00")
	capsuleDomain         = []byte("forgeos.aadm.decision-capsule.v1\x00")
	branchDomain          = []byte("forgeos.aadm.evaluation-branch.v1\x00")
	closureDomain         = []byte("forgeos.aadm.structural-replay-closure.v1\x00")
	decisionClosureDomain = []byte("forgeos.kernel-decision-reference-closure.v1\x00")
)

const capsuleResult = "STRUCTURALLY_VALID_DECISION_CAPSULE_V1 " +
	"(exact caller-supplied ADR-0090 closure and complete projection of the embedded " +
	"caller-supplied closure only; replay is validate/reseal/compare only; no effect " +
	"replay or history rewrite; all thirty-two replay attestations are false)"

const successMarker = "STRUCTURALLY_VALID_DECISION_CAPSULE_REPLAY_CLOSURE_V1 " +
	"(exact caller-supplied DecisionCapsule and separately sealed deterministic structural " +
	"comparison only; dedicated ReflectionReport ArtifactRefs are unresolved and attached only " +
	"by the outer closure; upstream ArtifactRefs remain opaque and uninterpreted; no model, rule " +
	"or world-state evaluation, effect replay, history rewrite, authorization, persistence, PDP " +
	"or controller; all thirty-two replay attestations are false)"
