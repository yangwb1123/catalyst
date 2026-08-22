package capabilitygrantcontract

import "fmt"

const frozenVocabularySHA256 = "a45de832e43ccdbebcb22f183575039d451594bfbc9ec713105c657a6adda49f"

type effectDescriptor struct {
	id          string
	allowed     []string
	required    []string
	profile     string
	restriction string
}

var frozenEffects = []effectDescriptor{
	effect("approval.decide", "approval_object", "governance_object"),
	effect("approval.request", "approval_object", "governance_object"),
	effect("knowledge.apply", "knowledge_object", "governance_object"),
	effect("knowledge.propose", "knowledge_object", "governance_object"),
	externalEffect("migration.apply", "artifact_environment", []string{"artifact", "environment"}),
	optionalEffect("migration.generate", "repo_emit_optional_environment"),
	effect("network.read", "network_origin", "network_origin"),
	effect("network.write", "network_origin", "network_origin"),
	effect("placement.plan", "target_query", "target_query"),
	effect("policy.propose", "policy_object", "governance_object"),
	effect("policy.write", "policy_object", "governance_object"),
	effect("process.exec", "command", "command"),
	externalEffect("release.execute", "artifact_environment", []string{"artifact", "environment"}),
	releasePlanEffect(),
	effect("repo.read", "repo_read", "repo_path"),
	effect("repo.write", "repo_write_exact", "repo_path"),
	effect("secrets.read", "secret_ref", "secret_ref"),
	effect("target.execute", "target", "target"),
	effect("target.inventory", "target_query", "target_query"),
	effect("target.probe", "target", "target"),
	effect("target.reserve", "target", "target"),
}

func effect(id, profile, kind string) effectDescriptor {
	return effectDescriptor{id, []string{kind}, []string{kind}, profile, "policy_controlled_default_deny"}
}

func externalEffect(id, profile string, kinds []string) effectDescriptor {
	return effectDescriptor{id, kinds, kinds, profile, "external_operator_only"}
}

func optionalEffect(id, profile string) effectDescriptor {
	return effectDescriptor{id, []string{"environment", "repo_path"}, []string{"repo_path"}, profile,
		"policy_controlled_default_deny"}
}

func releasePlanEffect() effectDescriptor {
	kinds := []string{"environment", "repo_path"}
	return effectDescriptor{"release.plan", kinds, kinds, "environment_repo_emit",
		"policy_controlled_default_deny"}
}

func findEffect(id string) (effectDescriptor, error) {
	for _, descriptor := range frozenEffects {
		if descriptor.id == id {
			return descriptor, nil
		}
	}
	return effectDescriptor{}, fmt.Errorf("unknown effect_id %q", id)
}
