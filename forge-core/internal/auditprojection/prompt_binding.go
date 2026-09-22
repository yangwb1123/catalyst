package auditprojection

import "forgeos/forge-core/internal/runtimebridge/model"

// ProjectCommittedPrompt binds the user Prompt returned by a successful Hub
// append to the ID-only change that the Hub committed in the same transaction.
//
// Project intentionally accepts only a change because it is also useful when
// replaying an owner-scoped change feed. Callers that still hold the append
// receipt should use this helper so a mismatched Prompt/change pair cannot be
// turned into accepted audit evidence. The helper remains pure: it performs no
// read, write, clock, network, enqueue, or publication effect.
func ProjectCommittedPrompt(
	owner model.Owner,
	prompt model.ConversationPrompt,
	change model.Change,
) (PromptAccepted, error) {
	if prompt.ID == "" || prompt.ConversationID == "" || prompt.Role != "user" ||
		prompt.ID != change.EntityID || prompt.ConversationID != change.ConversationID ||
		prompt.CreatedAtMS != change.CreatedAtMS {
		return PromptAccepted{}, ErrInvalidProjection
	}
	return Project(owner, change)
}
