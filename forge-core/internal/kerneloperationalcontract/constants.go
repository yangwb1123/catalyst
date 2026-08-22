package kerneloperationalcontract

const (
	canonicalization               = "forgeos.canonical-json/v1"
	artifactReceiptAPI             = "forgeos.artifact-receipt/v1"
	invocationAPI                  = "forgeos.capability-invocation/v1"
	eventAPI                       = "forgeos.interaction-event/v1"
	executionReceiptAPI            = "forgeos.execution-receipt/v1"
	closureAPI                     = "forgeos.kernel-operational-reference-closure/v1"
	artifactReceiptKind            = "ArtifactReceipt"
	invocationKind                 = "CapabilityInvocation"
	eventKind                      = "InteractionEvent"
	executionReceiptKind           = "ExecutionReceipt"
	closureKind                    = "KernelOperationalReferenceClosure"
	artifactReceiptDomain          = "forgeos.kernel.artifact-receipt.v1\x00"
	invocationDomain               = "forgeos.kernel.capability-invocation.v1\x00"
	eventDomain                    = "forgeos.kernel.interaction-event.v1\x00"
	executionReceiptDomain         = "forgeos.kernel.execution-receipt.v1\x00"
	closureDomain                  = "forgeos.kernel-operational-reference-closure.v1\x00"
	artifactReceiptPrefix          = "artifact-receipt-"
	invocationPrefix               = "capability-invocation-"
	eventPrefix                    = "interaction-event-"
	executionReceiptPrefix         = "execution-receipt-"
	closurePrefix                  = "kernel-operational-reference-closure-"
	maxArtifactReceiptBytes        = 262144
	maxInvocationBytes             = 524288
	maxEventBytes                  = 262144
	maxExecutionReceiptBytes       = 1048576
	maxClosureBytes                = 16777216
	maxArtifactRefBytes            = 16384
	maxJSONDepth                   = 16
	maxObjectFields                = 64
	maxArrayItems                  = 256
	maxStringBytes                 = 16384
	maxShortBytes                  = 160
	maxReferenceBytes              = 4096
	maxArtifacts                   = 256
	maxArtifactReceipts            = 64
	maxInvocations                 = 64
	maxEvents                      = 256
	maxExecutionReceipts           = 64
	maxAttempt                     = 64
	maxIOItems                     = 32
	maxReasonCodes                 = 32
	maxConfidenceMicros            = 1000000
	maxCallCount             int64 = 1000000000
	maxCostMicros            int64 = 1000000000000000
	maxElapsedMS             int64 = 86400000
	maxTokenCount            int64 = 1000000000
	maxNetworkBytes          int64 = 1073741824
	maxOutputBytes           int64 = 1073741824
)

const successMarker = "STRUCTURALLY_VALID_KERNEL_OPERATIONAL_REFERENCE_CLOSURE_V1 " +
	"(exact caller-supplied records and acyclic references only; no content provenance, " +
	"principal, Grant, or source/context/environment/policy binding authentication; no " +
	"authorization, permission, event append, persistence, transition, execution, outcome, " +
	"completion, effect, or usage measurement attestation)"
