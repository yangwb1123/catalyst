package platformcorecontract

const (
	// CanonicalizationV1 is the exact JSON profile emitted by v1 writers.
	CanonicalizationV1 = "forge.canonical-json/v1"
	// EnvelopeVersionV1 is independent from each payload schema version.
	EnvelopeVersionV1   = int64(1)
	maxArtifactRefBytes = 16 * 1024
	maxEnvelopeBytes    = 256 * 1024
	maxPayloadBytes     = 32 * 1024
	maxExtensionsBytes  = 8 * 1024
	maxJSONDepth        = 12
	maxObjectFields     = 64
	maxArrayItems       = 256
	maxStringBytes      = 16 * 1024
	maxExtensionFields  = 16
	maxArtifactBytes    = int64(1) << 40
	maxUnixMilliseconds = int64(253402300799999)
)

const (
	artifactDigestDomain = "forge.platform.artifact-ref.v1\x00"
	commandDigestDomain  = "forge.platform.command-envelope.v1\x00"
	eventDigestDomain    = "forge.platform.event-envelope.v1\x00"
)

const (
	entityAction          EntityType = "action"
	entityActor           EntityType = "actor"
	entityArtifact        EntityType = "artifact"
	entityAttempt         EntityType = "attempt"
	entityChange          EntityType = "change"
	entityObjective       EntityType = "objective"
	entityProject         EntityType = "project"
	entityProjectSnapshot EntityType = "project_snapshot"
	entityReceipt         EntityType = "receipt"
	entitySession         EntityType = "session"
	entitySpace           EntityType = "space"
	entityTurn            EntityType = "turn"
	entityWorkGraph       EntityType = "work_graph"
	entityWorkItem        EntityType = "work_item"
)

const (
	actorAgent   ActorType = "agent"
	actorHarness ActorType = "harness"
	actorHuman   ActorType = "human"
	actorService ActorType = "service"
	actorSystem  ActorType = "system"
)

const (
	sourceAppServer      SourceComponent = "app_server"
	sourceControlPlane   SourceComponent = "control_plane"
	sourceHarness        SourceComponent = "harness"
	sourceLegacyImporter SourceComponent = "legacy_importer"
	sourceRuntime        SourceComponent = "runtime"
)
