// Package capabilityregistry validates and resolves the authority-neutral
// Capability Registry v1 from explicit bytes. It never reads repository
// content refs, selects implementations, or grants runtime authority.
package capabilityregistry

const (
	registryAPIVersion   = "forgeos.capability-registry/v1"
	entryAPIVersion      = "forgeos.capability-registry-entry/v1"
	contractAPIVersion   = "forgeos.capability-contract/v1"
	requestAPIVersion    = "forgeos.capability-registry-declared-resolution-request/v1"
	assessmentAPIVersion = "forgeos.capability-registry-declared-resolution/v1"
	canonicalization     = "forgeos.canonical-json/v1"

	registryDigestDomain   = "forgeos.capability-registry.v1\x00"
	entryDigestDomain      = "forgeos.capability-registry-entry.v1\x00"
	contractDigestDomain   = "forgeos.capability-contract.v1\x00"
	contentSetDigestDomain = "forgeos.capability-registry.content-set.v1\x00"
	requestDigestDomain    = "forgeos.capability-registry-declared-resolution-request.v1\x00"
	assessmentDigestDomain = "forgeos.capability-registry-declared-resolution.v1\x00"

	maxRegistryBytes   = 16 << 20
	maxEntryBytes      = 1 << 20
	maxContractBytes   = 512 << 10
	maxContentSetBytes = 4 << 20
	maxRequestBytes    = 256 << 10
	maxAssessmentBytes = 256 << 10
	maxGoldenBytes     = 32 << 20
	maxJSONDepth       = 16
	maxObjectFields    = 64
	maxArrayItems      = 256
	maxStringBytes     = 16 << 10
	maxIdentifierBytes = 160
	maxRepoPathBytes   = 4096
	maxContentFiles    = 256

	effectVocabularySHA256 = "a45de832e43ccdbebcb22f183575039d451594bfbc9ec713105c657a6adda49f"
	capabilityID           = "local-go-package-impact-prescan"
	capabilityVersion      = "1"

	// Registry v1 admits exactly this independently constructed singleton
	// profile; future bytes require a new profile/version rather than fallback.
	pinnedRegistrySHA256 = "23b9acd4133598cd1404c78c71f694b4a99c398652e95c21896a507be5ecacf4"

	assessmentResult = "RESOLVED_DECLARED_CAPABILITY_REFERENCE_ONLY (no registry or owner authentication, rule or gate applicability, proof satisfaction, test pass, implementation availability, Grant activation, authorization, permission, invocation, runtime routing, persistence, transition, execution, or effect attestation)"
)

var relationKeys = []string{
	"domain", "effects", "failure_modes", "identity", "input_schemas",
	"not_applicable", "observability", "output_schemas", "permission_requirements",
	"postconditions", "preconditions", "proof_obligations", "quality_gates",
	"risk_floor", "rollback_or_compensation", "rules", "trigger",
}

var nonIdentityRelationKeys = []string{
	"domain", "effects", "failure_modes", "input_schemas", "not_applicable",
	"observability", "output_schemas", "permission_requirements", "postconditions",
	"preconditions", "proof_obligations", "quality_gates", "risk_floor",
	"rollback_or_compensation", "rules", "trigger",
}

var frozenEffects = map[string]string{
	"approval.decide": "approval_object", "approval.request": "approval_object",
	"knowledge.apply": "knowledge_object", "knowledge.propose": "knowledge_object",
	"migration.apply": "artifact_environment", "migration.generate": "repo_emit_optional_environment",
	"network.read": "network_origin", "network.write": "network_origin",
	"placement.plan": "target_query", "policy.propose": "policy_object",
	"policy.write": "policy_object", "process.exec": "command",
	"release.execute": "artifact_environment", "release.plan": "environment_repo_emit",
	"repo.read": "repo_read", "repo.write": "repo_write_exact",
	"secrets.read": "secret_ref", "target.execute": "target",
	"target.inventory": "target_query", "target.probe": "target",
	"target.reserve": "target",
}

var frozenContentSets = map[string]struct {
	count  int
	bytes  int64
	digest string
	suffix string
}{
	"forge-core/internal/goimpactprescan": {
		18, 83167, "549818abbb33737c9198607e2d43b56efef50b476aac507446d5501f86b4de22", ".go",
	},
	"harness/local_go_package_impact_prescan_contract": {
		8, 35348, "effade443429146470a13b55341c73228fa8d718e88be47532260663fb534bd4", ".py",
	},
	"<explicit>": {
		3, 27574, "3d7a072ffcaa6a222ae42ef6ac1b6135029ad5158fc38c70ce35f5afb3a28100", "",
	},
}
