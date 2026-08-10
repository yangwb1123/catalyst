package localcommandobservationproducer

import (
	"fmt"
	"path/filepath"
	"strings"

	commandcontract "forgeos/forge-core/internal/commandobservationevidencecontract"
	"forgeos/forge-core/internal/gitworktreesource"
)

func validateToolBinding(
	manifest ToolManifest,
	command commandProfile,
	observation commandcontract.Observation,
) error {
	if err := validateToolManifest(manifest, command.Argv[0]); err != nil {
		return err
	}
	_, digest, err := digestManifest(toolDigestDomain, manifest)
	if err != nil || digest != observation.Command.ToolSnapshotSHA256 {
		return fmt.Errorf("production tool manifest binding mismatch")
	}
	return nil
}

func validateToolManifest(manifest ToolManifest, requested string) error {
	if manifest.APIVersion != ToolAPIVersion || manifest.Canonicalization != Canonicalization ||
		manifest.ProfileID != toolProfileID || manifest.RequestedPath != requested {
		return fmt.Errorf("production tool manifest fixed fields drifted")
	}
	if manifest.Bytes < 0 || manifest.Bytes > maxIndividualFileBytes || !validDigest(manifest.SHA256) ||
		manifest.Mode < 0 || manifest.Mode > 0o777 || manifest.Mode&0o111 == 0 {
		return fmt.Errorf("production tool manifest file facts are invalid")
	}
	if err := validateText("requested_path", manifest.RequestedPath, false); err != nil {
		return err
	}
	if err := validateAbsoluteToolPath("resolved_path", manifest.ResolvedPath); err != nil {
		return err
	}
	if err := validateAbsoluteToolPath("final_path", manifest.FinalPath); err != nil {
		return err
	}
	if manifest.SymlinkHops == nil || len(manifest.SymlinkHops) > maxSymlinkHops {
		return fmt.Errorf("production tool symlink_hops are invalid")
	}
	for _, hop := range manifest.SymlinkHops {
		if err := validateAbsoluteToolPath("symlink path", hop.Path); err != nil {
			return err
		}
		if err := validateText("symlink target", hop.Target, false); err != nil {
			return err
		}
	}
	return validateToolSymlinkChain(manifest)
}

func validateToolSymlinkChain(manifest ToolManifest) error {
	if filepath.Base(manifest.ResolvedPath) != manifest.RequestedPath {
		return fmt.Errorf("production tool resolved path does not match requested basename")
	}
	candidate, seen := manifest.ResolvedPath, make(map[string]struct{})
	for _, hop := range manifest.SymlinkHops {
		if _, exists := seen[hop.Path]; exists {
			return fmt.Errorf("production tool symlink_hops contain a cycle")
		}
		seen[hop.Path] = struct{}{}
		if hop.Path != candidate && !strings.HasPrefix(candidate, hop.Path+string(filepath.Separator)) {
			return fmt.Errorf("production tool symlink hop is not on the resolution path")
		}
		remainder := strings.TrimPrefix(strings.TrimPrefix(candidate, hop.Path), string(filepath.Separator))
		target := hop.Target
		if !filepath.IsAbs(target) {
			target = filepath.Join(filepath.Dir(hop.Path), target)
		}
		candidate = filepath.Clean(filepath.Join(target, remainder))
	}
	if candidate != manifest.FinalPath {
		return fmt.Errorf("production tool symlink chain does not resolve to final_path")
	}
	return nil
}

func validateAbsoluteToolPath(label, value string) error {
	if err := validateText(label, value, false); err != nil {
		return err
	}
	if !filepath.IsAbs(value) || filepath.Clean(value) != value {
		return fmt.Errorf("%s must be a normalized absolute path", label)
	}
	return nil
}

func validateSourceBinding(manifest SourceManifest, observation commandcontract.Observation) error {
	if err := gitworktreesource.Validate(manifest, observation.Source.SourceTreeSHA256); err != nil ||
		manifest.SourceRevision != observation.Source.SourceRevision {
		return fmt.Errorf("production source manifest binding mismatch")
	}
	return nil
}
