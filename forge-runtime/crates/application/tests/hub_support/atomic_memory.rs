use std::path::Path;

use forge_runtime_domain::{GroupProjectMember, HubEntity, HubStoreError, Project};

use super::{MemoryState, conflict, project_name, require_group, same_member};

pub(super) fn link_project_path(
    state: &mut MemoryState,
    group_id: &str,
    path: &Path,
    role: &str,
    key: &str,
) -> Result<GroupProjectMember, HubStoreError> {
    require_group(state, group_id)?;
    if let Some(result) = replay_path_member(state, group_id, path, role, key) {
        return result;
    }
    let existing_project = state
        .projects
        .iter()
        .find(|item| item.path == path)
        .cloned();
    reject_existing_link(state, group_id, role, existing_project.as_ref())?;
    let project = existing_project.unwrap_or_else(|| add_project(state, path));
    let (_, added_at_ms) = state.identity("member");
    let member = GroupProjectMember {
        group_id: group_id.into(),
        project_id: project.id,
        role: role.into(),
        added_at_ms,
    };
    state.member_keys.push((key.into(), member.clone()));
    state.members.push(member.clone());
    Ok(member)
}

fn reject_existing_link(
    state: &MemoryState,
    group_id: &str,
    role: &str,
    project: Option<&Project>,
) -> Result<(), HubStoreError> {
    let Some(project) = project else {
        return Ok(());
    };
    let Some(member) = state
        .members
        .iter()
        .find(|item| item.group_id == group_id && item.project_id == project.id)
    else {
        return Ok(());
    };
    same_member(member, group_id, &project.id, role)?;
    Err(conflict(HubEntity::GroupProjectMember))
}

fn add_project(state: &mut MemoryState, path: &Path) -> Project {
    let (id, created_at_ms) = state.identity("project");
    let project = Project {
        id,
        name: project_name(path),
        path: path.to_path_buf(),
        created_at_ms,
    };
    state.projects.push(project.clone());
    project
}

fn replay_path_member(
    state: &MemoryState,
    group_id: &str,
    path: &Path,
    role: &str,
    key: &str,
) -> Option<Result<GroupProjectMember, HubStoreError>> {
    let member = &state.member_keys.iter().find(|(item, _)| item == key)?.1;
    let project = state
        .projects
        .iter()
        .find(|item| item.id == member.project_id);
    Some(match project {
        Some(project)
            if member.group_id == group_id && member.role == role && project.path == path =>
        {
            Ok(member.clone())
        }
        _ => Err(conflict(HubEntity::GroupProjectMember)),
    })
}
