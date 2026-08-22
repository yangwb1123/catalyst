package workintentcontract

type WorkIntent struct {
	APIVersion       string       `json:"api_version"`
	Attestations     Attestations `json:"attestations"`
	Binding          Binding      `json:"binding"`
	Canonicalization string       `json:"canonicalization"`
	DeclaredAtUnixMS int64        `json:"declared_at_unix_ms"`
	DeclaredOwner    *Principal   `json:"declared_owner"`
	Freshness        string       `json:"freshness"`
	Intent           Intent       `json:"intent"`
	Kind             string       `json:"kind"`
	Materiality      Materiality  `json:"materiality"`
	Origin           Origin       `json:"origin"`
	References       References   `json:"references"`
	Requester        Principal    `json:"requester"`
	Status           string       `json:"status"`
	WorkIntentID     string       `json:"work_intent_id"`
	WorkIntentSHA256 string       `json:"work_intent_sha256"`
}

type Attestations struct {
	Approval            bool `json:"approval_attestation"`
	Authentication      bool `json:"authentication_attestation"`
	Authority           bool `json:"authority_attestation"`
	Completion          bool `json:"completion_attestation"`
	Effect              bool `json:"effect_attestation"`
	Execution           bool `json:"execution_attestation"`
	Freshness           bool `json:"freshness_attestation"`
	Materiality         bool `json:"materiality_attestation"`
	Ownership           bool `json:"ownership_attestation"`
	Permission          bool `json:"permission_attestation"`
	Persistence         bool `json:"persistence_attestation"`
	ReferenceResolution bool `json:"reference_resolution_attestation"`
	Scope               bool `json:"scope_attestation"`
	Truth               bool `json:"truth_attestation"`
}

type Binding struct {
	ChangeID  string  `json:"change_id"`
	ProjectID string  `json:"project_id"`
	RunID     *string `json:"run_id"`
}

type Principal struct {
	AuthorityDomain string `json:"authority_domain"`
	PrincipalID     string `json:"principal_id"`
	PrincipalType   string `json:"principal_type"`
}

type Intent struct {
	DeadlineUnixMS      *int64   `json:"deadline_unix_ms"`
	ExternalConstraints []string `json:"external_constraints"`
	Goal                string   `json:"goal"`
	NonGoals            []string `json:"non_goals"`
	OpenQuestions       []string `json:"open_questions"`
	Scope               []string `json:"scope"`
	SuccessSignals      []string `json:"success_signals"`
	WorkType            string   `json:"work_type"`
}

type Materiality struct {
	Basis string `json:"basis"`
	Level string `json:"level"`
}

type Origin struct {
	OriginKind string  `json:"origin_kind"`
	OriginRef  *string `json:"origin_ref"`
}

type References struct {
	ClaimRecordRefs                []RecordRef           `json:"claim_record_refs"`
	EvidenceRecordRefs             []RecordRef           `json:"evidence_record_refs"`
	LocalArtifactDeclarations      []ArtifactDeclaration `json:"local_artifact_declarations"`
	LocalSourceSnapshotDeclaration *SourceSnapshot       `json:"local_source_snapshot_declaration"`
}

type RecordRef struct {
	CanonicalSHA256 string `json:"canonical_sha256"`
	RecordID        string `json:"record_id"`
}

type ArtifactDeclaration struct {
	ArtifactKind   string `json:"artifact_kind"`
	ArtifactRef    string `json:"artifact_ref"`
	ArtifactSHA256 string `json:"artifact_sha256"`
}

type SourceSnapshot struct {
	SnapshotID     string `json:"snapshot_id"`
	SnapshotSHA256 string `json:"snapshot_sha256"`
	SnapshotType   string `json:"snapshot_type"`
}
