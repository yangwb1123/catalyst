#!/usr/bin/env python3
"""Canonical Backend Engineering v1 vocabulary shared by policy/package checks."""

POLICY_REF = ".agent/engineering/backend-decision-gates.yml"
SCHEMA_REF = ".agent/eval/backend-decision-package.schema.yml"
STANDARD_REF = "docs/design/ai-engineering-os/backend-decision-standard.md"

# Raw-byte pins are updated only with an explicit v1 governance change. They
# prevent prose or nested-schema weakening that a shape-only validator misses.
POLICY_SHA256 = "c395aada1fe81343c0c25c9d3554282560f00bf90d6609b09468dead17d726be"
SCHEMA_SHA256 = "8022d4cdfd76a3ab5375b5510dac4e0f80b77d960cc850678b2be91c3d57b07d"

TRIGGER_FLOORS = {
    "backend_behavior": ("L1", "W1_standard"),
    "core_domain": ("L2", "W2_assured"),
    "persistence_schema": ("L2", "W2_assured"),
    "public_contract": ("L2", "W2_assured"),
    "event_contract": ("L2", "W2_assured"),
    "external_effect": ("L2", "W2_assured"),
    "concurrency": ("L2", "W2_assured"),
    "multi_tenant": ("L3", "W3_systemic"),
    "security_privacy": ("L3", "W3_systemic"),
    "performance_capacity": ("L2", "W2_assured"),
    "background_job": ("L2", "W2_assured"),
    "distributed_boundary": ("L2", "W2_assured"),
    "financial_effect": ("L4", "W3_systemic"),
    "authentication_core": ("L4", "W3_systemic"),
    "production_data": ("L4", "W3_systemic"),
    "destructive_migration": ("L4", "W3_systemic"),
}
TRIGGERS = set(TRIGGER_FLOORS)
MATERIALITY_RANK = {"L1": 1, "L2": 2, "L3": 3, "L4": 4}
WORKFLOW_RANK = {"W1_standard": 1, "W2_assured": 2, "W3_systemic": 3}

SEQUENCE = [
    "understand_goal", "model_invariants", "establish_boundaries",
    "design_persistence", "design_contracts", "select_algorithms",
    "design_consistency", "design_dependencies", "design_security",
    "allocate_nfr_budgets", "design_operations", "implement", "verify",
    "release_evolve",
]
DIMENSION_OWNERS = {
    "business_invariants": "domain-modeling",
    "domain_ownership": "domain-modeling",
    "model_boundaries": "backend-engineering",
    "persistence": "data-modeling-transactions",
    "contracts_compatibility": "api-contract-design",
    "algorithms_structures": "backend-engineering",
    "transactions_concurrency": "data-modeling-transactions",
    "distributed_reliability": "distributed-reliability-design",
    "security_tenancy_privacy": "secure-coding",
    "performance_capacity": "performance-capacity",
    "observability_operations": "observability-engineering",
    "migration_recovery": "data-migration-lifecycle",
    "evolution_economics": "architecture-tradeoff",
    "verification_release": "backend-engineering",
}
DIMENSION_PROOF_TYPES = {
    "business_invariants": {"requirement_trace", "invariant_catalog", "counterexample_tests"},
    "domain_ownership": {"context_map", "aggregate_catalog", "ownership_matrix"},
    "model_boundaries": {"model_map", "mapper_contract", "exposure_review"},
    "persistence": {"persistence_design", "data_dictionary", "query_index_matrix", "integrity_constraints"},
    "contracts_compatibility": {"contract_diff", "consumer_inventory", "compatibility_tests"},
    "algorithms_structures": {"complexity_analysis", "access_pattern_matrix", "benchmark_plan"},
    "transactions_concurrency": {"transaction_matrix", "concurrency_strategy", "race_and_idempotency_tests"},
    "distributed_reliability": {"failure_mode_matrix", "retry_timeout_budget", "recovery_tests"},
    "security_tenancy_privacy": {"threat_model", "authorization_matrix", "data_classification", "security_tests"},
    "performance_capacity": {"capacity_model", "baseline_measurement", "scale_trigger_table"},
    "observability_operations": {"telemetry_plan", "alert_runbook", "operator_recovery_path"},
    "migration_recovery": {"migration_plan", "rollback_or_forward_fix", "restore_evidence"},
    "evolution_economics": {"option_matrix", "adr", "ownership_and_cost_review", "exit_plan"},
    "verification_release": {"test_matrix", "execution_receipts", "release_observation_plan"},
}
TRIGGER_DIMENSIONS = {
    "backend_behavior": {"business_invariants", "model_boundaries", "verification_release"},
    "core_domain": {"business_invariants", "domain_ownership", "model_boundaries", "verification_release"},
    "persistence_schema": {"persistence", "transactions_concurrency", "migration_recovery", "verification_release"},
    "public_contract": {"contracts_compatibility", "security_tenancy_privacy", "verification_release"},
    "event_contract": {"contracts_compatibility", "distributed_reliability", "observability_operations", "verification_release"},
    "external_effect": {"transactions_concurrency", "distributed_reliability", "observability_operations", "verification_release"},
    "concurrency": {"transactions_concurrency", "distributed_reliability", "verification_release"},
    "multi_tenant": {"persistence", "contracts_compatibility", "security_tenancy_privacy", "verification_release"},
    "security_privacy": {"security_tenancy_privacy", "observability_operations", "verification_release"},
    "performance_capacity": {"algorithms_structures", "performance_capacity", "observability_operations", "verification_release"},
    "background_job": {"distributed_reliability", "observability_operations", "verification_release"},
    "distributed_boundary": {"contracts_compatibility", "distributed_reliability", "observability_operations", "evolution_economics"},
    "financial_effect": {"business_invariants", "transactions_concurrency", "distributed_reliability", "security_tenancy_privacy", "verification_release"},
    "authentication_core": {"business_invariants", "contracts_compatibility", "security_tenancy_privacy", "observability_operations", "verification_release"},
    "production_data": {"persistence", "security_tenancy_privacy", "observability_operations", "migration_recovery", "verification_release"},
    "destructive_migration": {"persistence", "security_tenancy_privacy", "observability_operations", "migration_recovery", "evolution_economics", "verification_release"},
}

MODEL_ROLE_CONDITIONS = {
    "request_dto": "transport_boundary",
    "command": "behavior_or_write_boundary",
    "domain_model": "domain_complexity_requires_invariants",
    "persistence_model": "storage_boundary",
    "read_model": "query_projection_differs",
    "response_dto": "transport_boundary",
    "external_service_model": "external_boundary",
}
READINESS_DIMENSIONS = [
    "requirements", "architecture", "domain_and_persistence",
    "algorithms_and_performance", "concurrency_and_consistency",
    "network_and_dependencies", "security_and_privacy",
    "observability_and_operations", "testing_and_evidence",
    "release_and_recovery",
]
READINESS_PROOF_TYPES = {
    "requirements": {"requirement_trace"},
    "architecture": {"context_map", "option_matrix"},
    "domain_and_persistence": {"invariant_catalog", "persistence_design", "integrity_constraints"},
    "algorithms_and_performance": {"complexity_analysis", "baseline_measurement"},
    "concurrency_and_consistency": {"transaction_matrix", "race_and_idempotency_tests"},
    "network_and_dependencies": {"retry_timeout_budget", "recovery_tests"},
    "security_and_privacy": {"threat_model", "authorization_matrix", "security_tests"},
    "observability_and_operations": {"telemetry_plan", "alert_runbook", "operator_recovery_path"},
    "testing_and_evidence": {"test_matrix", "execution_receipts"},
    "release_and_recovery": {"release_observation_plan", "rollback_or_forward_fix", "restore_evidence"},
}
READINESS_DECISION_DEPENDENCIES = {
    "requirements": {"business_invariants"},
    "architecture": {"domain_ownership", "model_boundaries", "evolution_economics"},
    "domain_and_persistence": {"domain_ownership", "persistence"},
    "algorithms_and_performance": {"algorithms_structures", "performance_capacity"},
    "concurrency_and_consistency": {"transactions_concurrency"},
    "network_and_dependencies": {"distributed_reliability"},
    "security_and_privacy": {"security_tenancy_privacy"},
    "observability_and_operations": {"observability_operations"},
    "testing_and_evidence": {"verification_release"},
    "release_and_recovery": {"migration_recovery", "verification_release"},
}
IRREVERSIBLE_KINDS = {
    "database_identity", "data_ownership", "public_contract", "event_semantics",
    "tenant_model", "authorization_model", "service_boundary", "partition_strategy",
}
TRIGGER_IRREVERSIBLE_BINDINGS = {
    "public_contract": ("contracts_compatibility", "public_contract"),
    "event_contract": ("contracts_compatibility", "event_semantics"),
    "multi_tenant": ("security_tenancy_privacy", "tenant_model"),
    "authentication_core": ("security_tenancy_privacy", "authorization_model"),
    "distributed_boundary": ("evolution_economics", "service_boundary"),
    "destructive_migration": ("persistence", "database_identity"),
}
TOOL_PROOF_TYPES = {
    "baseline_measurement", "compatibility_tests", "counterexample_tests",
    "execution_receipts", "race_and_idempotency_tests", "recovery_tests",
    "restore_evidence", "security_tests",
}
SPECIAL_PROOF_TYPES = {"source_fact", "applicability_assessment", "independent_review", "assumption_verification"}
PROOF_TYPES = set().union(*DIMENSION_PROOF_TYPES.values(), *READINESS_PROOF_TYPES.values(), SPECIAL_PROOF_TYPES)
EVIDENCE_CLASSES = {"source_artifact", "tool_receipt", "review_receipt"}
EVIDENCE_CLASS_PROOF_TYPES = {
    "source_artifact": PROOF_TYPES - TOOL_PROOF_TYPES - {
        "assumption_verification", "applicability_assessment", "independent_review",
    },
    "tool_receipt": TOOL_PROOF_TYPES | {"assumption_verification"},
    "review_receipt": {"applicability_assessment", "independent_review"},
}
EVIDENCE_CLASS_PRODUCERS = {
    "source_artifact": {"user", "repository"},
    "tool_receipt": {"tool"},
    "review_receipt": {"operator"},
}
EVIDENCE_CLASS_RESULTS = {
    "source_artifact": {"observed"},
    "tool_receipt": {"passed", "failed", "not_executed", "inconclusive"},
    "review_receipt": {"observed"},
}
MAX_EVIDENCE_BYTES = 8 * 1024 * 1024
FORBIDDEN_DECISION_KEYS = {"completed", "accepted", "approved", "verdict", "gate_result"}
