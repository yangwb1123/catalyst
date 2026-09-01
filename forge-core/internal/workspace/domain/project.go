package domain

import core "forgeos/forge-core/internal/platformcorecontract"

const projectRegisteredSchema = "forge.workspace.project_registered"

// FoldProject reconstructs one immutable Project v1 from exact canonical events.
func FoldProject(events []*core.EventEnvelope) (Project, error) {
	event, err := creationEvent(events, "project", projectRegisteredSchema)
	if err != nil {
		return Project{}, err
	}
	if !projectScopeMatches(event) || event.SourceSnapshotRef != nil {
		return Project{}, invalidHistory("Project scope differs from aggregate")
	}
	fields := []string{
		"alias", "operation", "project_id", "root_path", "root_path_status", "space_id",
	}
	if err := exactPayload(event.Payload, fields...); err != nil {
		return Project{}, err
	}
	project, err := projectPayload(event)
	if err != nil {
		return Project{}, err
	}
	project.RegisteredBy = event.ActorRef
	project.RegisteredAtMS = event.OccurredAtUnixMS
	project.Version = 1
	return project, nil
}

func projectScopeMatches(event *core.EventEnvelope) bool {
	scope := event.ScopeRef
	return scope.SpaceID != "" && pointerEquals(scope.ProjectID, event.AggregateRef.EntityID) &&
		scope.ActionID == nil && scope.AttemptID == nil && scope.ChangeID == nil &&
		scope.ObjectiveID == nil && scope.ProjectSnapshotID == nil &&
		scope.SessionID == nil && scope.TurnID == nil &&
		scope.WorkGraphID == nil && scope.WorkItemID == nil
}

func projectPayload(event *core.EventEnvelope) (Project, error) {
	projectID, err := payloadString(event.Payload, "project_id")
	if err != nil || projectID != event.AggregateRef.EntityID {
		return Project{}, invalidHistory("Project payload identity differs")
	}
	spaceID, err := payloadString(event.Payload, "space_id")
	if err != nil || spaceID != event.ScopeRef.SpaceID {
		return Project{}, invalidHistory("Project payload Space differs")
	}
	operation, _ := payloadString(event.Payload, "operation")
	alias, aliasErr := payloadString(event.Payload, "alias")
	root, rootErr := payloadString(event.Payload, "root_path")
	status, statusErr := payloadString(event.Payload, "root_path_status")
	if operation != "register_project" || aliasErr != nil || rootErr != nil || statusErr != nil ||
		validateAlias(alias) != nil || validateRootPath(root) != nil ||
		status != RootPathDeclaredUnverified {
		return Project{}, invalidHistory("Project payload values differ")
	}
	return Project{
		ProjectID: projectID, SpaceID: spaceID, Alias: alias,
		RootPath: root, RootPathStatus: status,
	}, nil
}
