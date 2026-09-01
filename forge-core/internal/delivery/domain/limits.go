package domain

const (
	impactAssessmentRecordType = "forge.delivery.impact_assessment"
	maxTargetProjects          = 16
	maxComparedSnapshots       = 32
	maxConstraints             = 32
	maxSuccessMeasures         = 32
	maxCriteria                = 64
	maxWorkItems               = 128
	maxDependencyEdges         = 512
	maxItemListEntries         = 32
	maxUnixMilliseconds        = int64(253402300799999)
	maxAttempts                = int64(1024)
	maxDurationMS              = int64(2592000000)
	maxCostMicroUSD            = int64(1000000000000)
)
