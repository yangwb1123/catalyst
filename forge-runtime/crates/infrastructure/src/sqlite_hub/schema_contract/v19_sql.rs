pub(super) const MIGRATE_V18_TO_V19_SQL: &str =
    "CREATE TABLE group_agent_graph_scheduled_node_dispatch_lifecycles_v19 (
  id TEXT NOT NULL PRIMARY KEY
    CHECK(typeof(id) = 'text' AND length(CAST(id AS BLOB)) BETWEEN 1 AND 128),
  graph_run_id TEXT NOT NULL UNIQUE
    REFERENCES group_agent_graph_runs(id) ON DELETE RESTRICT
    CHECK(typeof(graph_run_id) = 'text'
      AND length(CAST(graph_run_id AS BLOB)) BETWEEN 1 AND 128),
  provider_request_id TEXT NOT NULL UNIQUE
    REFERENCES group_agent_graph_scheduled_node_provider_requests(id) ON DELETE RESTRICT
    CHECK(typeof(provider_request_id) = 'text'
      AND length(CAST(provider_request_id AS BLOB)) BETWEEN 1 AND 128),
  authorization_id TEXT NOT NULL UNIQUE
    CHECK(typeof(authorization_id) = 'text'
      AND length(CAST(authorization_id AS BLOB)) BETWEEN 1 AND 256),
  authorization_sha256 BLOB NOT NULL
    CHECK(typeof(authorization_sha256) = 'blob' AND length(authorization_sha256) = 32),
  provider_request_sha256 BLOB NOT NULL
    CHECK(typeof(provider_request_sha256) = 'blob' AND length(provider_request_sha256) = 32),
  request_body_blob BLOB NOT NULL
    CHECK(typeof(request_body_blob) = 'blob' AND length(request_body_blob) BETWEEN 1 AND 16777216),
  request_body_bytes INTEGER NOT NULL
    CHECK(typeof(request_body_bytes) = 'integer' AND request_body_bytes = length(request_body_blob)),
  project_lane_sha256 BLOB NOT NULL
    CHECK(typeof(project_lane_sha256) = 'blob' AND length(project_lane_sha256) = 32),
  node_id TEXT NOT NULL
    CHECK(typeof(node_id) = 'text' AND length(CAST(node_id AS BLOB)) BETWEEN 1 AND 128),
  attempt INTEGER NOT NULL CHECK(typeof(attempt) = 'integer' AND attempt = 1),
  claim_json BLOB NOT NULL
    CHECK(typeof(claim_json) = 'blob' AND length(claim_json) BETWEEN 1 AND 65536),
  claim_json_bytes INTEGER NOT NULL
    CHECK(typeof(claim_json_bytes) = 'integer' AND claim_json_bytes = length(claim_json)),
  active_lane_json BLOB NOT NULL
    CHECK(typeof(active_lane_json) = 'blob' AND length(active_lane_json) BETWEEN 1 AND 65536),
  active_lane_json_bytes INTEGER NOT NULL
    CHECK(typeof(active_lane_json_bytes) = 'integer' AND active_lane_json_bytes = length(active_lane_json)),
  release_control_json BLOB NOT NULL
    CHECK(typeof(release_control_json) = 'blob' AND length(release_control_json) BETWEEN 1 AND 67108864),
  release_control_json_bytes INTEGER NOT NULL
    CHECK(typeof(release_control_json_bytes) = 'integer' AND release_control_json_bytes = length(release_control_json)),
  authorization_json BLOB NOT NULL
    CHECK(typeof(authorization_json) = 'blob' AND length(authorization_json) BETWEEN 1 AND 1048576),
  authorization_json_bytes INTEGER NOT NULL
    CHECK(typeof(authorization_json_bytes) = 'integer' AND authorization_json_bytes = length(authorization_json)),
  pricing_json BLOB NOT NULL
    CHECK(typeof(pricing_json) = 'blob' AND length(pricing_json) BETWEEN 1 AND 16384),
  pricing_json_bytes INTEGER NOT NULL
    CHECK(typeof(pricing_json_bytes) = 'integer' AND pricing_json_bytes = length(pricing_json)),
  claim_event_json BLOB NOT NULL
    CHECK(typeof(claim_event_json) = 'blob' AND length(claim_event_json) BETWEEN 1 AND 65536),
  claim_event_json_bytes INTEGER NOT NULL
    CHECK(typeof(claim_event_json_bytes) = 'integer' AND claim_event_json_bytes = length(claim_event_json)),
  status TEXT NOT NULL
    CHECK(typeof(status) = 'text'
      AND status IN ('claimed','terminalized','quarantined','adjudicated')),
  lane_active INTEGER NOT NULL
    CHECK(typeof(lane_active) = 'integer'
      AND ((status = 'claimed' AND lane_active = 1)
        OR (status IN ('terminalized','quarantined','adjudicated') AND lane_active = 0))),
  artifact_json BLOB
    CHECK(artifact_json IS NULL OR (typeof(artifact_json) = 'blob' AND length(artifact_json) BETWEEN 1 AND 1048576)),
  artifact_json_bytes INTEGER
    CHECK((artifact_json IS NULL AND artifact_json_bytes IS NULL)
      OR (typeof(artifact_json_bytes) = 'integer' AND artifact_json_bytes = length(artifact_json))),
  terminal_control_json BLOB
    CHECK(terminal_control_json IS NULL OR (typeof(terminal_control_json) = 'blob' AND length(terminal_control_json) BETWEEN 1 AND 67108864)),
  terminal_control_json_bytes INTEGER
    CHECK((terminal_control_json IS NULL AND terminal_control_json_bytes IS NULL)
      OR (typeof(terminal_control_json_bytes) = 'integer' AND terminal_control_json_bytes = length(terminal_control_json))),
  terminal_receipt_json BLOB
    CHECK(terminal_receipt_json IS NULL OR (typeof(terminal_receipt_json) = 'blob' AND length(terminal_receipt_json) BETWEEN 1 AND 65536)),
  terminal_receipt_json_bytes INTEGER
    CHECK((terminal_receipt_json IS NULL AND terminal_receipt_json_bytes IS NULL)
      OR (typeof(terminal_receipt_json_bytes) = 'integer' AND terminal_receipt_json_bytes = length(terminal_receipt_json))),
  created_at_ms INTEGER NOT NULL CHECK(typeof(created_at_ms) = 'integer' AND created_at_ms >= 0),
  terminalized_at_ms INTEGER
    CHECK(terminalized_at_ms IS NULL OR (typeof(terminalized_at_ms) = 'integer' AND terminalized_at_ms >= created_at_ms))
);
INSERT INTO group_agent_graph_scheduled_node_dispatch_lifecycles_v19
  SELECT id,graph_run_id,provider_request_id,authorization_id,authorization_sha256,
         provider_request_sha256,request_body_blob,request_body_bytes,project_lane_sha256,
         node_id,attempt,claim_json,claim_json_bytes,active_lane_json,active_lane_json_bytes,
         release_control_json,release_control_json_bytes,authorization_json,authorization_json_bytes,
         pricing_json,pricing_json_bytes,claim_event_json,claim_event_json_bytes,status,lane_active,
         artifact_json,artifact_json_bytes,terminal_control_json,terminal_control_json_bytes,
         terminal_receipt_json,terminal_receipt_json_bytes,created_at_ms,terminalized_at_ms
  FROM group_agent_graph_scheduled_node_dispatch_lifecycles;
DROP TABLE group_agent_graph_scheduled_node_dispatch_lifecycles;
ALTER TABLE group_agent_graph_scheduled_node_dispatch_lifecycles_v19
  RENAME TO group_agent_graph_scheduled_node_dispatch_lifecycles;
CREATE UNIQUE INDEX group_agent_graph_scheduled_node_dispatch_lifecycles_project_lane_active
  ON group_agent_graph_scheduled_node_dispatch_lifecycles(project_lane_sha256)
  WHERE lane_active = 1;
CREATE INDEX group_agent_graph_scheduled_node_dispatch_lifecycles_created
  ON group_agent_graph_scheduled_node_dispatch_lifecycles(created_at_ms DESC, id DESC);
PRAGMA user_version = 19;";
