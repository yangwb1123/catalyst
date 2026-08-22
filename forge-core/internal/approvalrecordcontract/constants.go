package approvalrecordcontract

const (
	approvalAPI                = "forgeos.approval-record/v1"
	requestAPI                 = "forgeos.approval-record-declared-assessment-request/v1"
	assessmentAPI              = "forgeos.approval-record-declared-assessment/v1"
	canonicalization           = "forgeos.canonical-json/v1"
	recordKind                 = "ApprovalRecord"
	assessmentMode             = "authority_neutral_declared_approval_only"
	effectVocabularyHash       = "a45de832e43ccdbebcb22f183575039d451594bfbc9ec713105c657a6adda49f"
	maxRecordBytes             = 1024 * 1024
	maxTargetBytes             = 1024 * 1024
	maxRequestBytes            = 2 * 1024 * 1024
	maxAssessmentBytes         = 256 * 1024
	maxShortBytes              = 160
	maxProofBytes              = 16 * 1024
	maxTTLMillis         int64 = 86_400_000
	approvalDomain             = "forgeos.approval-record.v1\x00"
	targetDomain               = "forgeos.approval-declared-target.v1\x00"
	requestDomain              = "forgeos.approval-record-declared-assessment-request.v1\x00"
	assessmentDomain           = "forgeos.approval-record-declared-assessment.v1\x00"
)

const assessedDeclarationsOnly = "ASSESSED_APPROVAL_DECLARATIONS_ONLY (no approver or authority authentication, attestation or SoD proof verification, condition or RiskAcceptance validation, revocation evaluation, policy decision, effective approval, authorization, permission, persistence, transition, execution, or effect attestation)"

var effects = []string{
	"approval.decide", "approval.request", "knowledge.apply", "knowledge.propose",
	"migration.apply", "migration.generate", "network.read", "network.write",
	"placement.plan", "policy.propose", "policy.write", "process.exec",
	"release.execute", "release.plan", "repo.read", "repo.write", "secrets.read",
	"target.execute", "target.inventory", "target.probe", "target.reserve",
}

var distinctions = []string{
	"approver_not_implementer", "approver_not_requester", "approver_not_subject",
}
