package knowledgeupdateproposalcontract

import "fmt"

var targetKeys = []string{
	"bindings", "capability_grant_ref", "knowledge_scope", "mutations", "proposer",
	"record_set_sha256", "task_binding",
}

func declaredTarget(proposal map[string]any) (map[string]any, error) {
	if err := validateProposal(proposal); err != nil {
		return nil, err
	}
	target := make(map[string]any, len(targetKeys))
	for _, key := range targetKeys {
		target[key] = cloneValue(proposal[key])
	}
	if err := validateTarget(target); err != nil {
		return nil, err
	}
	return target, nil
}

func validateTarget(target map[string]any) error {
	if err := validateCanonicalByteLimit(target, maxTargetBytes, "knowledge update declared target"); err != nil {
		return err
	}
	if err := requireKeys(target, targetKeys...); err != nil {
		return fmt.Errorf("knowledge update declared target: %w", err)
	}
	bindings, bindingErr := objectValue(target, "bindings")
	grantRef, grantErr := objectValue(target, "capability_grant_ref")
	scope, scopeErr := objectValue(target, "knowledge_scope")
	proposer, proposerErr := objectValue(target, "proposer")
	task, taskErr := objectValue(target, "task_binding")
	mutations, mutationErr := arrayValue(target, "mutations")
	if bindingErr != nil || grantErr != nil || scopeErr != nil || proposerErr != nil ||
		taskErr != nil || mutationErr != nil {
		return fmt.Errorf("knowledge update declared target contains invalid field types")
	}
	checks := []func() error{
		func() error { return validateBindings(bindings, "bindings") },
		func() error { return validateGrantRef(grantRef, "capability_grant_ref") },
		func() error { return validateKnowledgeScope(scope, "knowledge_scope") },
		func() error { return validatePrincipal(proposer, "proposer") },
		func() error { return validateTaskBinding(task, "task_binding") },
		func() error { return validateMutationsShape(mutations) },
	}
	for _, check := range checks {
		if err := check(); err != nil {
			return err
		}
	}
	digest, err := stringValue(target, "record_set_sha256")
	if err != nil || validateHash(digest, "record_set_sha256") != nil {
		return fmt.Errorf("record_set_sha256 is invalid")
	}
	return nil
}

func targetDigest(target map[string]any) (string, error) {
	if err := validateTarget(target); err != nil {
		return "", err
	}
	return digestValue(targetDomain, target)
}
