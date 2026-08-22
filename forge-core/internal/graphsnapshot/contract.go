package graphsnapshot

const (
	apiVersion               = "forgeos.governance.local-go-graph-snapshot-projection/v1"
	requestVersion           = "forgeos.governance.local-go-graph-snapshot-projection-request/v1"
	snapshotVersion          = "forgeos.governance.graph-snapshot/v1"
	canonicalization         = "forgeos.canonical-json/v1"
	profileID                = "adr-0053-selected-go-module-lexical-partial-graph-snapshot-v1"
	testSourceAPIVersion     = "forgeos.governance.local-go-test-source-graph-snapshot-projection/v1"
	testSourceRequestVersion = "forgeos.governance.local-go-test-source-graph-snapshot-projection-request/v1"
	testSourceProfileID      = "adr-0053-selected-go-module-lexical-package-test-source-partial-graph-snapshot-v1"

	resultText           = "PROJECTED_PARTIAL_GRAPH_SNAPSHOT (exact ADR-0053 selected-module lexical module/package subgraph only; coverage partial and system/freshness unknown; no selected-build, cross-surface completeness, truth, authority, completion, persistence, execution, impact, or effect attestation)"
	testSourceResultText = "PROJECTED_PARTIAL_GRAPH_SNAPSHOT (exact ADR-0053 selected-module lexical module/package/test-source subgraph only; test nodes are source sets, not tests or outcomes; coverage partial and system/freshness unknown; no selected-build, cross-surface completeness, truth, authority, completion, persistence, execution, verification, impact, or effect attestation)"

	requestDomain            = "forgeos.governance.local-go-graph-snapshot-projection-request.v1"
	sourceIdentityDomain     = "forgeos.governance.graph-snapshot-source-identity.v1"
	sourceDomain             = "forgeos.governance.graph-snapshot-source.v1"
	extractorIdentityDomain  = "forgeos.governance.graph-snapshot-extractor-identity.v1"
	extractorDomain          = "forgeos.governance.graph-snapshot-extractor.v1"
	nodeIdentityDomain       = "forgeos.governance.graph-snapshot-node-identity.v1"
	nodeDomain               = "forgeos.governance.graph-snapshot-node.v1"
	edgeIdentityDomain       = "forgeos.governance.graph-snapshot-edge-identity.v1"
	edgeDomain               = "forgeos.governance.graph-snapshot-edge.v1"
	unresolvedNodeIDDomain   = "forgeos.governance.graph-snapshot-unresolved-node-identity.v1"
	unresolvedNodeDomain     = "forgeos.governance.graph-snapshot-unresolved-node.v1"
	unresolvedEdgeIDDomain   = "forgeos.governance.graph-snapshot-unresolved-edge-identity.v1"
	unresolvedEdgeDomain     = "forgeos.governance.graph-snapshot-unresolved-edge.v1"
	sourceSetDomain          = "forgeos.governance.graph-snapshot-source-set.v1"
	extractorSetDomain       = "forgeos.governance.graph-snapshot-extractor-set.v1"
	nodeSetDomain            = "forgeos.governance.graph-snapshot-node-set.v1"
	edgeSetDomain            = "forgeos.governance.graph-snapshot-edge-set.v1"
	unresolvedNodeSetDomain  = "forgeos.governance.graph-snapshot-unresolved-node-set.v1"
	unresolvedEdgeSetDomain  = "forgeos.governance.graph-snapshot-unresolved-edge-set.v1"
	crosswalkSetDomain       = "forgeos.governance.graph-snapshot-adr-0062-node-crosswalk-set.v1"
	coverageDomain           = "forgeos.governance.graph-snapshot-coverage.v1"
	snapshotIdentityDomain   = "forgeos.governance.graph-snapshot-identity.v1"
	snapshotDomain           = "forgeos.governance.graph-snapshot.v1"
	envelopeDomain           = "forgeos.governance.local-go-graph-snapshot-projection.v1"
	testSourceRequestDomain  = "forgeos.governance.local-go-test-source-graph-snapshot-projection-request.v1"
	testSourceEnvelopeDomain = "forgeos.governance.local-go-test-source-graph-snapshot-projection.v1"
	adr0062NodeDomain        = "forgeos.governance.local-go-package-impact-prescan-node.v1"

	maxGraphBytes                  = 16 << 20
	maxBase64Bytes                 = 22_369_622
	maxRequestBytes                = 24 << 20
	maxSnapshotBytes               = 64 << 20
	maxEnvelopeBytes               = 96 << 20
	maxNodes                       = 16_385
	maxPackages                    = 16_384
	maxEdges                       = 81_920
	maxDependencies                = 65_536
	maxUnresolvedNodes             = 17_408
	maxLocators                    = 16_384
	maxAggregateLocators           = 131_072
	maxTestSourceNodes             = 32_769
	maxTestSourceEdges             = 98_304
	maxTestSourceAggregateLocators = 132_097
)

type projectionProfile struct {
	apiVersion           string
	envelopeDomain       string
	includeTestSources   bool
	maxAggregateLocators int
	maxEdges             int
	maxNodes             int
	maxReasonCodes       int
	profileID            string
	requestDomain        string
	requestVersion       string
	resultText           string
	systemUnknownReasons []string
}

var legacyProfile = projectionProfile{
	apiVersion: apiVersion, envelopeDomain: envelopeDomain,
	maxAggregateLocators: maxAggregateLocators, maxEdges: maxEdges,
	maxNodes: maxNodes, maxReasonCodes: 20, profileID: profileID,
	requestDomain: requestDomain, requestVersion: requestVersion,
	resultText: resultText, systemUnknownReasons: systemUnknownReasons,
}

var testSourceProfile = projectionProfile{
	apiVersion: testSourceAPIVersion, envelopeDomain: testSourceEnvelopeDomain,
	includeTestSources: true, maxAggregateLocators: maxTestSourceAggregateLocators,
	maxEdges: maxTestSourceEdges, maxNodes: maxTestSourceNodes, maxReasonCodes: 24,
	profileID: testSourceProfileID, requestDomain: testSourceRequestDomain,
	requestVersion: testSourceRequestVersion, resultText: testSourceResultText,
	systemUnknownReasons: testSourceSystemUnknownReasons,
}

var freshnessReasons = []string{
	"source_observation_clock_unauthenticated",
	"source_observation_not_atomic_snapshot",
	"zero_duration_expiry_no_freshness_attestation",
}

var systemUnknownReasons = []string{
	"adr_owner_policy_surfaces_not_observed",
	"api_event_contract_surfaces_not_observed",
	"business_domain_surfaces_not_observed",
	"call_and_runtime_reachability_not_observed",
	"data_and_migration_surfaces_not_observed",
	"deployment_and_operations_surfaces_not_observed",
	"freshness_not_attested",
	"other_language_module_package_surfaces_not_observed",
	"selected_build_semantics_not_observed",
	"test_and_verification_surfaces_not_observed",
}

var testSourceSystemUnknownReasons = []string{
	"adr_owner_policy_surfaces_not_observed",
	"api_event_contract_surfaces_not_observed",
	"business_domain_surfaces_not_observed",
	"call_and_runtime_reachability_not_observed",
	"data_and_migration_surfaces_not_observed",
	"deployment_and_operations_surfaces_not_observed",
	"freshness_not_attested",
	"other_language_module_package_surfaces_not_observed",
	"selected_build_semantics_not_observed",
	"test_execution_and_verification_outcomes_not_observed",
}

var surfaceNames = []string{
	"adr_decision", "api_event_contract", "business_domain",
	"data_schema_migration", "deployment_environment", "go_module_package_lexical",
	"operations_runtime_signal", "other_language_module_package", "owner_policy",
	"symbol_call_runtime", "test_verification",
}
