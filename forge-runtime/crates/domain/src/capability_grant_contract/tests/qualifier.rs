use super::super::*;
use super::{fixture, reseal_grant, reseal_request};

#[test]
fn migration_generate_environment_is_an_exact_clause_qualifier() {
    let cases = [
        (
            Some("dev"),
            Some("dev"),
            ScopeRelation::CoveredByDeclaration,
        ),
        (Some("dev"), None, ScopeRelation::OutsideDeclaredScope),
        (
            Some("dev"),
            Some("other"),
            ScopeRelation::OutsideDeclaredScope,
        ),
        (None, Some("dev"), ScopeRelation::OutsideDeclaredScope),
    ];
    for (declared_environment, requested_environment, expected) in cases {
        let (vocabulary, request) =
            migration_generate_request(declared_environment, requested_environment);
        let assessment =
            evaluate_declared_assessment(&vocabulary, &request).expect("assess migration.generate");
        assert_eq!(assessment.relations.scope, expected);
        let reasons = if expected == ScopeRelation::CoveredByDeclaration {
            Vec::new()
        } else {
            vec![ReasonCode::ScopeNotCovered]
        };
        assert_eq!(assessment.reason_codes, reasons);
    }
}

fn migration_generate_request(
    declared_environment: Option<&str>,
    requested_environment: Option<&str>,
) -> (EffectVocabulary, DeclaredAssessmentRequest) {
    let fixture = fixture();
    let mut request = fixture.assessment_request;
    request.grant.scope = GrantScope {
        allow: vec![ScopeClause {
            resources: migration_resources(declared_environment),
        }],
        deny: Vec::new(),
        effect_id: EffectId::MigrationGenerate,
    };
    reseal_grant(&mut request.grant);
    request.requested_action.effect_id = EffectId::MigrationGenerate;
    request.requested_action.resources = migration_resources(requested_environment);
    reseal_request(&mut request);
    (fixture.effect_vocabulary, request)
}

fn migration_resources(environment_id: Option<&str>) -> Vec<ScopeResource> {
    let mut resources = Vec::new();
    if let Some(environment_id) = environment_id {
        resources.push(ScopeResource::Environment {
            environment_class: EnvironmentClass::Development,
            environment_id: environment_id.into(),
            environment_sha256: "b".repeat(64),
        });
    }
    resources.push(ScopeResource::RepoPath {
        path_match: PathMatch::Exact,
        path: "out/migration.sql".into(),
    });
    resources
}
