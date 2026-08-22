package planningownership

const (
	canonicalization       = "forgeos.canonical-json/v1"
	requestAPIVersion      = "forgeos.planning-capability-ownership-projection-request/v1"
	projectionAPIVersion   = "forgeos.planning-capability-ownership-projection/v1"
	requestKind            = "PlanningCapabilityOwnershipProjectionRequest"
	projectionKind         = "PlanningCapabilityOwnershipProjection"
	requestDigestDomain    = "forgeos.planning-capability-ownership-projection-request.v1\x00"
	bindingDigestDomain    = "forgeos.planning-capability-ownership-binding.v1\x00"
	projectionDigestDomain = "forgeos.planning-capability-ownership-projection.v1\x00"
	positiveResult         = "PROJECTED_PLANNING_CAPABILITY_OWNERSHIP_ONLY (complete declared primary-owner coverage and logical adapter references for the supplied planning sources only; no file existence, Skill availability, Registry mutation, authentication, authorization, permission, invocation, routing, execution, persistence, transition, or effect attestation)"
	projectionMode         = "planning_only_declared_ownership_and_logical_adapter_refs"
	catalogDocumentName    = "capability-catalog.v1.yml"
	mappingDocumentName    = "capability-skill-map.v1.yml"
)

const (
	maxRequestBytes       = 1_048_576
	maxProjectionBytes    = 4_194_304
	maxBindingBytes       = 16_384
	maxCatalogSourceBytes = 262_144
	maxMappingSourceBytes = 131_072
	maxYAMLDepth          = 16
	maxYAMLFields         = 64
	maxYAMLItems          = 512
	maxYAMLTokens         = 65_536
	maxYAMLCollections    = 4_096
	maxYAMLScalarBytes    = 16_384
	maxJSONDepth          = 16
	maxJSONFields         = 64
	maxJSONItems          = 512
	maxJSONStringBytes    = 349_528
	maxIdentifierBytes    = 160
	maxAdapterRefBytes    = 192
)

var catalogTopFields = []string{
	"api_version", "authority_semantics", "canonical_vocabulary",
	"control_plane_joins", "decision_ref", "executable",
	"extension_decision_refs", "gates", "kind", "nodes", "risk_levels",
	"runtime_note", "status", "universal_node_contract",
}

var catalogNodeFields = []string{
	"activities", "authority", "capabilities", "entry_criteria", "escalation",
	"exit_criteria", "forbidden", "handoff", "id", "inputs", "memory_updates",
	"name", "outputs", "owner_lens", "purpose", "quality_gates", "rules",
}

var mappingTopFields = []string{
	"api_version", "executable", "kind", "mapping_rules", "packages",
	"skill_specification", "source_catalog", "status",
}

var mappingPackageFields = []string{"implementation_wave", "includes", "skill"}
