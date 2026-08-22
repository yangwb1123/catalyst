package projectsnapshot

const (
	canonicalization = "forgeos.canonical-json/v1"
	requestVersion   = "forgeos.governance.local-project-source-snapshot-request/v1"
	envelopeVersion  = "forgeos.governance.local-project-source-snapshot-production/v1"
	snapshotVersion  = "forgeos.project-source-snapshot/v1"
	manifestVersion  = "forgeos.project-source-manifest/v1"
	coverageVersion  = "forgeos.project-source-coverage/v1"
	profileID        = "local-git-worktree-bounded-sensitive-path-exclusion-v1"
	pathPolicyID     = "forgeos.project-source-sensitive-path-policy/v1"
	extractorID      = "local-git-worktree-project-source-snapshot"
	extractorVersion = "1"
	envelopeKind     = "LocalProjectSourceSnapshotProduction"
	snapshotKind     = "ProjectSourceSnapshot"
	positiveResult   = "CAPTURED_BOUNDED_LOCAL_PROJECT_SOURCE_OBSERVATION (the collector worktree-leaf reader did not open paths matched by its fixed path policy; Git and repository-metadata access are outside that guarantee; this is not an atomic, current, complete, secret-free, authenticated, authorized, persistent, or effect-attesting project or graph snapshot)"
)

const (
	consistencyValue = "bounded_interval_two_endpoint_exact_match"
	unknownValue     = "unknown"
)

const (
	gitIdentityAttestation  = "unauthenticated_local_path_binary"
	gitLocalConfigIsolation = "best_effort_hardened_environment_and_flags_repository_config_unauthenticated"
	gitNetworkContainment   = "not_provided"
)
