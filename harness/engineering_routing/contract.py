"""Canonical v1 context-route identities, floors, and budgets."""

CONTEXT_FIELDS = {
    "changed_path", "task_type", "materiality", "workflow_profile", "capability_id",
}
CONTEXT_LANES = {"instruction", "trusted_context", "untrusted_data"}
CONTEXT_BUDGET_MAX = {"max_files": 24, "max_total_bytes": 524288}
REQUIRED_DENY_GLOBS = {"**/.env", "**/.env.*", "**/*private-key*", "**/.ssh/**"}
BASE_REQUIRED_FLOORS = {
    ".agent/AGENTS.md": {"lane": "instruction", "max_bytes": 65536},
    ".agent/PROJECT.md": {"lane": "trusted_context", "max_bytes": 131072},
    ".agent/project.yml": {"lane": "trusted_context", "max_bytes": 65536},
}
REQUIRED_ROUTE_IDS = {
    "governance", "architecture-boundary", "implementation", "security-change",
    "user-experience", "release-boundary", "data-and-contract", "backend-runtime",
}
ROUTE_INCLUDE_FLOORS = {
    "governance": {
        ".agent/engineering/rules.yml": "instruction",
        ".agent/skills/context-engineering.md": "instruction",
        ".agent/skills/policy-authority.md": "instruction",
        ".agent/skills/knowledge-graph-curation.md": "instruction",
        ".agent/skills/change-impact-cost-risk.md": "instruction",
        ".agent/skills/adr-governance.md": "instruction",
        ".agent/skills/capability-ownership-projection.md": "instruction",
        ".agent/policies/modes.yml": "trusted_context",
        "docs/contracts/context-package-v1.schema.json": "trusted_context",
        "docs/contracts/capability-grant-v1.schema.json": "trusted_context",
        "docs/contracts/approval-record-v1.schema.json": "trusted_context",
        "docs/contracts/bootstrap-grant-issuance-v1.schema.json": "trusted_context",
        "docs/contracts/bootstrap-repo-read-execution-v1.schema.json": "trusted_context",
        "docs/contracts/transition-receipt-v1.schema.json": "trusted_context",
        "docs/contracts/knowledge-update-proposal-v1.schema.json": "trusted_context",
        "docs/contracts/local-go-package-impact-prescan-v1.schema.json": "trusted_context",
        "docs/contracts/graph-snapshot-v1.schema.json": "trusted_context",
        "docs/contracts/architecture-decision-record-v2.schema.json": "trusted_context",
        "docs/contracts/planning-capability-ownership-projection-v1.schema.json": "trusted_context",
    },
    "architecture-boundary": {
        ".agent/skills/clean-architecture.md": "instruction",
        ".agent/skills/domain-modeling.md": "instruction",
        ".agent/skills/architecture-tradeoff.md": "instruction",
        ".agent/skills/knowledge-graph-curation.md": "instruction",
        ".agent/skills/change-impact-cost-risk.md": "instruction",
        ".agent/skills/adr-governance.md": "instruction",
        ".agent/skills/capability-ownership-projection.md": "instruction",
        ".agent/engineering/backend-decision-gates.yml": "instruction",
        ".agent/eval/backend-decision-package.schema.yml": "trusted_context",
        ".arch/rules.yaml": "trusted_context",
        "docs/contracts/local-go-package-impact-prescan-v1.schema.json": "trusted_context",
        "docs/contracts/graph-snapshot-v1.schema.json": "trusted_context",
        "docs/contracts/architecture-decision-record-v2.schema.json": "trusted_context",
        "docs/contracts/planning-capability-ownership-projection-v1.schema.json": "trusted_context",
    },
    "implementation": {
        ".agent/skills/testing.md": "instruction",
        ".agent/workflows/build.yml": "trusted_context",
    },
    "security-change": {
        ".agent/skills/security-review.md": "instruction",
        ".agent/skills/secure-coding.md": "instruction",
        ".agent/engineering/backend-decision-gates.yml": "instruction",
        ".agent/eval/backend-decision-package.schema.yml": "trusted_context",
        "harness/secret-scan.mjs": "trusted_context",
    },
    "user-experience": {
        ".agent/skills/code-review.md": "instruction",
        ".agent/skills/information-interaction-design.md": "instruction",
        ".agent/skills/design-system-accessibility.md": "instruction",
        ".agent/skills/ui-geometry.md": "instruction",
        ".agent/skills/frontend-client-engineering.md": "instruction",
        ".agent/skills/frontend-code-architecture.md": "instruction",
        ".agent/engineering/frontend-design-gates.yml": "instruction",
        ".agent/engineering/frontend-code-architecture.yml": "instruction",
        ".agent/engineering/frontend-profiles.yml": "trusted_context",
        ".agent/eval/frontend-design-package.schema.yml": "trusted_context",
        ".arch/frontend-architecture.v1.json": "trusted_context",
        ".arch/frontend-architecture-baseline.v1.json": "trusted_context",
        ".arch/frontend-architecture-waivers.v1.json": "trusted_context",
        "docs/design/ai-engineering-os/frontend-design-standard.md": "trusted_context",
        "docs/design/ai-engineering-os/frontend-code-architecture-standard.md": "trusted_context",
    },
    "release-boundary": {
        ".agent/workflows/deploy.yml": "instruction",
        ".agent/workflows/rollback.yml": "instruction",
    },
    "data-and-contract": {
        ".agent/skills/testing.md": "instruction",
        ".agent/skills/data-modeling-transactions.md": "instruction",
        ".agent/skills/data-migration-lifecycle.md": "instruction",
        ".agent/skills/api-contract-design.md": "instruction",
        ".agent/engineering/backend-decision-gates.yml": "instruction",
        ".agent/eval/backend-decision-package.schema.yml": "trusted_context",
        ".agent/eval/completion-evidence.schema.yml": "trusted_context",
    },
    "backend-runtime": {
        ".agent/skills/backend-engineering.md": "instruction",
        ".agent/skills/distributed-reliability-design.md": "instruction",
        ".agent/skills/performance-capacity.md": "instruction",
        ".agent/skills/observability-engineering.md": "instruction",
        ".agent/engineering/backend-decision-gates.yml": "instruction",
        ".agent/eval/backend-decision-package.schema.yml": "trusted_context",
    },
}
ROUTE_MATCH_FLOORS = {
    "governance": {"op": "any", "predicates": [
        {"field": "changed_path", "operator": "glob_any", "values": [".agent/**", "harness/**"]},
        {"field": "task_type", "operator": "in", "values": ["governance", "agent_engineering"]},
    ]},
    "architecture-boundary": {"op": "any", "predicates": [
        {"field": "changed_path", "operator": "glob_any", "values": ["**/domain/**", "**/application/**", "**/infrastructure/**", "**/interfaces/**"]},
        {"field": "capability_id", "operator": "in", "values": ["modular-architecture", "architecture-conformance", "change-impact-analysis"]},
    ]},
    "implementation": {"op": "any", "predicates": [
        {"field": "changed_path", "operator": "glob_any", "values": ["src/**", "lib/**", "app/**", "packages/**"]},
        {"field": "task_type", "operator": "in", "values": ["feature", "bug_fix", "refactor"]},
    ]},
    "security-change": {"op": "any", "predicates": [
        {"field": "changed_path", "operator": "glob_any", "values": ["**/auth/**", "**/security/**", "**/identity/**"]},
        {"field": "task_type", "operator": "in", "values": ["security", "authentication", "authorization"]},
        {"field": "materiality", "operator": "in", "values": ["L4"]},
    ]},
    "user-experience": {"op": "any", "predicates": [
        {"field": "changed_path", "operator": "glob_any", "values": ["**/ui/**", "**/components/**", "**/pages/**", "**/screens/**", "**/views/**", "**/routes/**", "**/widgets/**", "**/features/**", "**/entities/**", "**/shared/ui/**", "**/shared/api/**", "**/styles/**", "**/theme/**", "**/tokens/**", "**/*.tsx", "**/*.jsx", "**/*.vue", "**/*.dart", "**/*.css", "**/*.scss", "**/*.sass", "**/*.less"]},
        {"field": "capability_id", "operator": "in", "values": ["information-architecture", "interaction-design", "content-design", "visual-design", "design-system", "accessibility", "usability-testing", "frontend-engineering", "client-engineering"]},
    ]},
    "release-boundary": {"op": "any", "predicates": [
        {"field": "changed_path", "operator": "glob_any", "values": ["docs/release/**", ".agent/workflows/deploy.yml", ".agent/workflows/rollback.yml"]},
        {"field": "task_type", "operator": "in", "values": ["deploy", "release", "rollback", "production_change"]},
    ]},
    "data-and-contract": {"op": "any", "predicates": [
        {"field": "changed_path", "operator": "glob_any", "values": ["**/migrations/**", "**/schema/**", "**/database/**", "**/db/**", "**/sql/**", "**/dao/**", "**/persistence/**", "**/repositories/**", "**/entities/**", "**/models/**", "**/api/**", "**/contracts/**", "**/proto/**", "**/events/**"]},
        {"field": "capability_id", "operator": "in", "values": ["data-modeling", "schema-review", "query-index-analysis", "transaction-design", "migration-engineering", "data-quality", "api-design", "event-contract", "compatibility", "idempotency", "error-modeling", "contract-testing"]},
    ]},
    "backend-runtime": {"op": "any", "predicates": [
        {"field": "changed_path", "operator": "glob_any", "values": ["**/internal/**", "**/services/**", "**/controllers/**", "**/handlers/**", "**/models/**", "**/repositories/**", "**/persistence/**", "**/clients/**", "**/jobs/**", "**/workers/**", "**/queues/**", "**/cache/**"]},
        {"field": "capability_id", "operator": "in", "values": ["backend-engineering", "distributed-systems", "concurrency", "observability", "benchmarking"]},
    ]},
}
SELECTION_FIELDS = {
    "strategy", "route_order", "match_snapshot", "path_semantics", "max_files",
    "max_total_bytes", "on_required_missing", "on_required_overflow",
    "on_optional_overflow", "required_metadata", "omit_log_required",
    "untrusted_content_behavior", "secret_redaction_required", "merge",
    "deny_globs", "base_required",
}
