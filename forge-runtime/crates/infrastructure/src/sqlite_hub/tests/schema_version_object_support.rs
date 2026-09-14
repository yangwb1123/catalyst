pub(super) const DROP_V29_CONTROLLER_SQL: &str = "DROP TABLE IF EXISTS pending_run_intent_events;
     DROP TABLE IF EXISTS pending_run_intents;
     DROP INDEX IF EXISTS project_execution_consent_active_lookup;
     DROP TABLE IF EXISTS project_execution_consent_events;
     DROP TABLE IF EXISTS project_execution_consent_grants;
     DROP TABLE IF EXISTS conversation_owner_change_rows;
     DROP TABLE IF EXISTS conversation_owner_change_heads;
     DROP INDEX IF EXISTS conversation_changes_conversation_cursor;
     DROP INDEX IF EXISTS conversation_owners_principal_conversation;
     DROP INDEX IF EXISTS conversation_owners_principal;
     DROP TABLE IF EXISTS conversation_owners;
     DROP TABLE IF EXISTS conversation_changes;
     DROP TABLE IF EXISTS conversation_change_heads;
     DROP TABLE IF EXISTS conversation_change_state;
     DROP TABLE IF EXISTS conversation_change_baselines;
     DROP TABLE group_agent_scheduled_graph_controller_events;
     DROP TABLE group_agent_scheduled_graph_controllers;";
pub(super) const DROP_V30_CONVERSATION_CHANGES_SQL: &str = "DROP TABLE conversation_changes;
     DROP TABLE conversation_change_heads;
     DROP TABLE conversation_change_state;
     DROP TABLE conversation_change_baselines;";
pub(super) const DROP_V31_CONVERSATION_OWNERS_SQL: &str =
    "DROP INDEX IF EXISTS conversation_owners_principal;
     DROP TABLE IF EXISTS conversation_owners;";
pub(super) const DROP_V32_OWNER_CURSOR_OBJECTS_SQL: &str =
    "DROP TABLE IF EXISTS conversation_owner_change_rows;
     DROP TABLE IF EXISTS conversation_owner_change_heads;
     DROP INDEX IF EXISTS conversation_changes_conversation_cursor;
     DROP INDEX IF EXISTS conversation_owners_principal_conversation;";
pub(super) const DROP_V33_PROJECT_CONSENT_OBJECTS_SQL: &str =
    "DROP INDEX IF EXISTS project_execution_consent_active_lookup;
     DROP TABLE IF EXISTS project_execution_consent_events;
     DROP TABLE IF EXISTS project_execution_consent_grants;";
pub(super) const DROP_V34_PENDING_RUN_INTENT_OBJECTS_SQL: &str =
    "DROP TABLE IF EXISTS pending_run_intent_events;
     DROP TABLE IF EXISTS pending_run_intents;
     DROP INDEX IF EXISTS pending_run_intents_owner_conversation_page;";
pub(super) const V29_CONTROLLER_OBJECTS: &[&str] = &[
    "group_agent_scheduled_graph_controllers",
    "group_agent_scheduled_graph_controllers_schedule",
    "group_agent_scheduled_graph_controller_events",
];
pub(super) const V30_CHANGE_OBJECTS: &[&str] = &[
    "conversation_change_baselines",
    "conversation_changes",
    "conversation_changes_event_entity",
    "conversation_changes_conversation_version",
    "conversation_change_state",
    "conversation_change_heads",
];
pub(super) const V31_OWNER_OBJECTS: &[&str] =
    &["conversation_owners", "conversation_owners_principal"];
pub(super) const V32_OWNER_CURSOR_OBJECTS: &[&str] = &[
    "conversation_owner_change_heads",
    "conversation_owner_change_rows",
    "conversation_changes_conversation_cursor",
    "conversation_owners_principal_conversation",
];
pub(super) const V33_PROJECT_CONSENT_OBJECTS: &[&str] = &[
    "project_execution_consent_grants",
    "project_execution_consent_active_lookup",
    "project_execution_consent_events",
];
pub(super) const V34_PENDING_RUN_INTENT_OBJECTS: &[&str] = &[
    "pending_run_intents",
    "pending_run_intent_events",
    "pending_run_intents_owner_conversation_page",
];
