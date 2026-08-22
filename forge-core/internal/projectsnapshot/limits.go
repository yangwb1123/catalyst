package projectsnapshot

const (
	maxEnvelopeBytes      = 32 << 20
	maxFileBytes          = int64(64 << 20)
	maxGitExecutableBytes = int64(64 << 20)
	maxGitOutputBytes     = 32 << 20
	maxIgnoredPaths       = int64(262_144)
	maxManifestBytes      = 16 << 20
	maxExcludedEntries    = 4_096
	maxPathBytes          = 16_384
	maxPathComponents     = 256
	maxPathScalars        = 4_096
	maxShortTextBytes     = 1_024
	maxTotalBytes         = int64(1 << 30)
	maxUniverseEntries    = 16_384
)

const (
	pathDomain         = "forgeos.project-source-snapshot.path.v1"
	requestDomain      = "forgeos.project-source-snapshot.request.v1"
	entryDomain        = "forgeos.project-source-snapshot.entry-record.v1"
	exclusionDomain    = "forgeos.project-source-snapshot.exclusion-record.v1"
	entrySetDomain     = "forgeos.project-source-snapshot.entry-set.v1"
	exclusionSetDomain = "forgeos.project-source-snapshot.exclusion-set.v1"
	manifestDomain     = "forgeos.project-source-snapshot.source-manifest.v1"
	coverageDomain     = "forgeos.project-source-snapshot.coverage.v1"
	snapshotIDDomain   = "forgeos.project-source-snapshot.snapshot-identity.v1"
	snapshotDomain     = "forgeos.project-source-snapshot.snapshot-record.v1"
	envelopeDomain     = "forgeos.project-source-snapshot.envelope.v1"
)

func lowerHex(value string) bool {
	if value == "" {
		return false
	}
	for _, character := range value {
		if character < '0' || character > '9' {
			if character < 'a' || character > 'f' {
				return false
			}
		}
	}
	return true
}
