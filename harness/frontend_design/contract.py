#!/usr/bin/env python3
"""Canonical Frontend Design v1 vocabulary for policy and package checks."""

POLICY_REF = ".agent/engineering/frontend-design-gates.yml"
PROFILE_REF = ".agent/engineering/frontend-profiles.yml"
SCHEMA_REF = ".agent/eval/frontend-design-package.schema.yml"
STANDARD_REF = "docs/design/ai-engineering-os/frontend-design-standard.md"

# Filled only by an explicit v1 governance update after canonical YAML changes.
POLICY_SHA256 = "1420a35a9c59e39f09f05631fc56bbc50041c13736f62eefec15f65877ec3eb2"
PROFILE_SHA256 = "bb688bb987945106ca2bd53383d73a77d58fcb14a877babf3606dd173224a76e"
SCHEMA_SHA256 = "2058b45ddfdd8f028e3142fb3235379a026597a9ebf527cb9cb3f383dee03cf6"

TRIGGER_FLOORS = {
    "frontend_behavior": ("L1", "W1_standard"),
    "component_change": ("L1", "W1_standard"),
    "page_or_route": ("L2", "W2_assured"),
    "form_or_table": ("L2", "W2_assured"),
    "business_workflow": ("L2", "W2_assured"),
    "shared_token_or_component": ("L3", "W3_systemic"),
    "responsive_adaptive": ("L2", "W2_assured"),
    "accessibility": ("L2", "W2_assured"),
    "visual_regression": ("L2", "W2_assured"),
    "cross_platform_ui": ("L2", "W2_assured"),
    "localization_or_rtl": ("L2", "W2_assured"),
    "public_brand_surface": ("L2", "W2_assured"),
    "data_intensive": ("L2", "W2_assured"),
    "immersive_3d": ("L2", "W2_assured"),
    "multi_role_permission": ("L3", "W3_systemic"),
    "high_risk_action": ("L3", "W3_systemic"),
    "authentication_or_payment": ("L4", "W3_systemic"),
    "destructive_user_data": ("L4", "W3_systemic"),
    "regulated_commitment": ("L4", "W3_systemic"),
    "safety_critical_surface": ("L4", "W3_systemic"),
}
TRIGGERS = set(TRIGGER_FLOORS)
MATERIALITY_RANK = {"L1": 1, "L2": 2, "L3": 3, "L4": 4}
WORKFLOW_RANK = {"W1_standard": 1, "W2_assured": 2, "W3_systemic": 3}

SEQUENCE = [
    "understand_business_goal", "classify_scenario",
    "map_information_architecture", "model_operation_flows",
    "model_state_permission_actions", "select_profile_and_pattern",
    "resolve_design_tokens", "define_responsive_and_input_behavior",
    "design_accessibility", "allocate_motion_and_performance_budgets",
    "map_framework_state_and_components", "implement",
    "verify_behavior", "verify_visuals", "release_and_evolve",
]

DIMENSION_OWNERS = {
    "business_task": "information-interaction-design",
    "scenario_profile": "information-interaction-design",
    "information_architecture": "information-interaction-design",
    "operation_flows": "information-interaction-design",
    "state_permission_action": "information-interaction-design",
    "async_feedback_recovery": "information-interaction-design",
    "design_language_tokens": "design-system-accessibility",
    "layout_component_composition": "design-system-accessibility",
    "responsive_adaptive_input": "frontend-client-engineering",
    "accessibility_semantics": "design-system-accessibility",
    "motion_performance": "frontend-client-engineering",
    "framework_state_data": "frontend-client-engineering",
    "interaction_verification": "frontend-client-engineering",
    "visual_quality": "design-system-accessibility",
}

DIMENSION_PROOF_TYPES = {
    "business_task": {"requirement_trace", "task_outcome_map"},
    "scenario_profile": {"scenario_classification", "profile_selection"},
    "information_architecture": {"information_architecture_map", "navigation_contract"},
    "operation_flows": {"operation_flow", "recovery_flow"},
    "state_permission_action": {"state_action_matrix", "permission_action_review"},
    "async_feedback_recovery": {"async_state_model", "feedback_recovery_matrix"},
    "design_language_tokens": {"token_resolution_manifest", "design_language_contract"},
    "layout_component_composition": {"component_mapping", "layout_specification"},
    "responsive_adaptive_input": {"responsive_matrix", "input_modality_matrix"},
    "accessibility_semantics": {"accessibility_spec", "accessibility_test_plan"},
    "motion_performance": {"motion_budget", "performance_budget"},
    "framework_state_data": {"framework_mapping", "state_ownership_map"},
    "interaction_verification": {"interaction_test_matrix", "interaction_execution_receipts"},
    "visual_quality": {"capture_manifest", "independent_visual_review"},
}

TRIGGER_DIMENSIONS = {
    "frontend_behavior": {"business_task", "framework_state_data", "interaction_verification"},
    "component_change": {"layout_component_composition", "accessibility_semantics", "framework_state_data", "interaction_verification"},
    "page_or_route": set(DIMENSION_OWNERS) - {"motion_performance"},
    "form_or_table": {"operation_flows", "state_permission_action", "async_feedback_recovery", "layout_component_composition", "accessibility_semantics", "framework_state_data", "interaction_verification"},
    "business_workflow": {"business_task", "operation_flows", "state_permission_action", "async_feedback_recovery", "interaction_verification"},
    "shared_token_or_component": {"design_language_tokens", "layout_component_composition", "accessibility_semantics", "visual_quality"},
    "responsive_adaptive": {"responsive_adaptive_input", "accessibility_semantics", "motion_performance", "interaction_verification", "visual_quality"},
    "accessibility": {"accessibility_semantics", "interaction_verification", "visual_quality"},
    "visual_regression": {"visual_quality"},
    "cross_platform_ui": {"scenario_profile", "design_language_tokens", "responsive_adaptive_input", "accessibility_semantics", "framework_state_data", "interaction_verification"},
    "localization_or_rtl": {"information_architecture", "responsive_adaptive_input", "accessibility_semantics", "interaction_verification", "visual_quality"},
    "public_brand_surface": {"scenario_profile", "design_language_tokens", "layout_component_composition", "responsive_adaptive_input", "accessibility_semantics", "motion_performance", "interaction_verification", "visual_quality"},
    "data_intensive": {"scenario_profile", "information_architecture", "operation_flows", "state_permission_action", "async_feedback_recovery", "layout_component_composition", "responsive_adaptive_input", "motion_performance", "framework_state_data", "interaction_verification", "visual_quality"},
    "immersive_3d": {"scenario_profile", "operation_flows", "async_feedback_recovery", "design_language_tokens", "layout_component_composition", "responsive_adaptive_input", "accessibility_semantics", "motion_performance", "framework_state_data", "interaction_verification", "visual_quality"},
    "multi_role_permission": {"business_task", "operation_flows", "state_permission_action", "accessibility_semantics", "interaction_verification"},
    "high_risk_action": {"business_task", "operation_flows", "state_permission_action", "async_feedback_recovery", "accessibility_semantics", "interaction_verification"},
    "authentication_or_payment": {"business_task", "operation_flows", "state_permission_action", "async_feedback_recovery", "accessibility_semantics", "framework_state_data", "interaction_verification"},
    "destructive_user_data": {"business_task", "operation_flows", "state_permission_action", "async_feedback_recovery", "interaction_verification"},
    "regulated_commitment": {"business_task", "operation_flows", "state_permission_action", "async_feedback_recovery", "accessibility_semantics", "interaction_verification"},
    "safety_critical_surface": {"business_task", "operation_flows", "state_permission_action", "async_feedback_recovery", "accessibility_semantics", "motion_performance", "interaction_verification"},
}

CLASSIFICATION_FIELDS = [
    "product_type", "business_domain", "page_pattern", "profile_id", "platform",
    "density", "motion_level", "operation_frequency", "data_density", "risk_level",
    "primary_user", "primary_task",
]
PLATFORMS = {"web_desktop", "web_responsive", "ios", "android", "cross_platform"}
DENSITIES = {"comfortable", "standard", "compact", "immersive", "fixed_canvas"}
MOTION_LEVELS = {0, 1, 2, 3}
FREQUENCIES = {"low", "medium", "high"}
RISK_LEVELS = {"low", "medium", "high", "critical"}

PROFILE_IDS = {
    "cms_editorial", "oa_workflow", "erp_mes_dense", "crm_relationship",
    "analytics_decision", "commerce_transaction", "mobile_task",
    "marketing_conversion", "immersive_story", "data_wall",
    "ai_agent_workspace", "generic_saas",
}
PROFILE_DEFAULTS = {
    "cms_editorial": ("standard", 1),
    "oa_workflow": ("standard", 1),
    "erp_mes_dense": ("compact", 0),
    "crm_relationship": ("standard", 1),
    "analytics_decision": ("standard", 1),
    "commerce_transaction": ("standard", 1),
    "mobile_task": ("comfortable", 1),
    "marketing_conversion": ("comfortable", 2),
    "immersive_story": ("immersive", 3),
    "data_wall": ("fixed_canvas", 1),
    "ai_agent_workspace": ("standard", 1),
    "generic_saas": ("standard", 1),
}
PAGE_PATTERN_IDS = {
    "list", "detail", "form", "workbench", "wizard", "editor", "approval",
    "dashboard", "landing", "immersive", "data_wall", "agent_chat",
    "visual_editor", "timeline",
}

READINESS_DIMENSIONS = [
    "business_and_task", "information_and_flow", "state_and_recovery",
    "visual_and_tokens", "responsive_and_input", "accessibility",
    "motion_and_performance", "framework_quality", "interaction_evidence",
    "visual_evidence",
]
READINESS_PROOF_TYPES = {
    "business_and_task": {"requirement_trace", "task_outcome_map"},
    "information_and_flow": {"information_architecture_map", "operation_flow"},
    "state_and_recovery": {"state_action_matrix", "feedback_recovery_matrix"},
    "visual_and_tokens": {"token_resolution_manifest", "layout_specification"},
    "responsive_and_input": {"responsive_matrix", "input_modality_matrix"},
    "accessibility": {"accessibility_spec", "accessibility_execution_receipts"},
    "motion_and_performance": {"motion_budget", "performance_measurement"},
    "framework_quality": {"framework_mapping", "static_execution_receipts"},
    "interaction_evidence": {"interaction_test_matrix", "interaction_execution_receipts"},
    "visual_evidence": {"capture_manifest", "visual_diff_receipts", "independent_visual_review"},
}
READINESS_DECISION_DEPENDENCIES = {
    "business_and_task": {"business_task", "scenario_profile"},
    "information_and_flow": {"information_architecture", "operation_flows"},
    "state_and_recovery": {"state_permission_action", "async_feedback_recovery"},
    "visual_and_tokens": {"design_language_tokens", "layout_component_composition"},
    "responsive_and_input": {"responsive_adaptive_input"},
    "accessibility": {"accessibility_semantics"},
    "motion_and_performance": {"motion_performance"},
    "framework_quality": {"framework_state_data"},
    "interaction_evidence": {"interaction_verification"},
    "visual_evidence": {"visual_quality"},
}

EXECUTION_PROOF_TYPES = {
    "interaction_execution_receipts", "accessibility_execution_receipts",
    "performance_measurement", "static_execution_receipts", "visual_diff_receipts",
    "capture_receipt", "assumption_verification",
}
REVIEW_PROOF_TYPES = {
    "permission_action_review", "independent_visual_review",
    "applicability_assessment", "independent_review", "profile_override_review",
}
SPECIAL_PROOF_TYPES = {"source_fact", "classification_fact", "capture_receipt"} \
    | REVIEW_PROOF_TYPES | {"assumption_verification", "architecture_decision_record"}
PROOF_TYPES = set().union(*DIMENSION_PROOF_TYPES.values(), *READINESS_PROOF_TYPES.values(), SPECIAL_PROOF_TYPES)
CLAIM_CLASS_PROOF_TYPES = {
    "source_observation": PROOF_TYPES - EXECUTION_PROOF_TYPES - REVIEW_PROOF_TYPES,
    "execution_observation": EXECUTION_PROOF_TYPES,
    "review_observation": REVIEW_PROOF_TYPES,
}
CLAIM_CLASS_CLAIMANTS = {
    "source_observation": {"user", "repository"},
    "execution_observation": {"tool"},
    "review_observation": {"operator"},
}
CLAIM_CLASS_RESULTS = {
    "source_observation": {"observed"},
    "execution_observation": {"passed", "failed", "not_executed", "inconclusive"},
    "review_observation": {"observed"},
}
CLAIM_CLASS_ARTIFACT_KINDS = {
    "source_observation": {"source"},
    "execution_observation": {"tool_output", "screenshot", "trace", "visual_diff", "accessibility_report"},
    "review_observation": {"review_output", "screenshot", "visual_diff"},
}

EFFECT_CLASSES = {
    "read_only", "reversible_write", "destructive", "external_commit",
    "legal_commitment", "financial_commitment",
}
HIGH_RISK_EFFECTS = EFFECT_CLASSES - {"read_only", "reversible_write"}
DECISION_KINDS = {"shared_token", "shared_component", "public_interaction_contract", "auth_surface", "other"}
VERIFICATION_KINDS = {"interaction", "capture"}
MAX_ARTIFACT_BYTES = 8 * 1024 * 1024
ASSUMPTION_BLOCK_THRESHOLD = 0.15
FORBIDDEN_KEYS = {"completed", "accepted", "approved", "verdict", "gate_result", "quality_passed"}
RISK_FLOORS = {
    "low": ("L1", "W1_standard"),
    "medium": ("L2", "W2_assured"),
    "high": ("L3", "W3_systemic"),
    "critical": ("L4", "W3_systemic"),
}
