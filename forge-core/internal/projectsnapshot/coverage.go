package projectsnapshot

func buildCoverage(sourceManifestSHA256 string, counts CoverageCounts) (Coverage, error) {
	surfaces := []CoverageSurface{
		{0, []string{"bounded_interval_observation", "writer_quiescence_not_provided"}, "unknown", "atomicity"},
		{0, []string{"configuration_classifier_not_run"}, "not_performed", "configuration_semantics"},
		{0, []string{"allowed_content_secret_absence_not_attested", "path_policy_only"}, "not_performed", "content_secret_scan"},
		{0, []string{"clock_not_authenticated", "current_head_not_attested"}, "unknown", "currentness"},
		{0, []string{"deployment_classifier_not_run"}, "not_performed", "deployment_semantics"},
		{0, []string{"freshness_not_assessed"}, "unknown", "freshness"},
		{0, []string{"git_binary_and_local_config_unauthenticated", "git_metadata_not_projected"}, "not_observed", "git_control_metadata"},
		{0, []string{"graph_extractor_not_run"}, "not_performed", "graph_topology"},
		{counts.IgnoredPathCount, []string{"content_and_locators_not_observed", "count_only"}, "partial", "ignored_paths"},
		{0, []string{"gitlinks_rejected", "nested_repository_semantics_not_inspected"}, "not_observed", "nested_repositories_and_submodules"},
		{counts.UntrackedCount, []string{"bounded_interval_not_atomic", "git_exclude_standard_applied", "ignored_paths_not_enumerated_as_source"}, "partial", "nonignored_untracked"},
		{counts.TrackedCount, []string{"bounded_interval_not_atomic", "git_stage_zero_only", "head_is_revision_hint_only", "nonordinary_index_flags_rejected", "worktree_bytes_not_index_blob"}, "partial", "tracked_worktree"},
	}
	value := Coverage{
		APIVersion: coverageVersion, Canonicalization: canonicalization,
		Counts: counts, SourceManifestSHA256: sourceManifestSHA256, Surfaces: surfaces,
	}
	digest, err := domainDigest(coverageDomain, value, maxManifestBytes)
	value.CoverageSHA256 = digest
	return value, err
}
