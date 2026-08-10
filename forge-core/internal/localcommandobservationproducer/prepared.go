package localcommandobservationproducer

import (
	"fmt"
	"reflect"
)

// validatePreparedProfiles rejects caller mutation between preparation and
// sealing. The type remains package-private so Run is the only production
// boundary; callers cannot inject a fabricated capture through the builder.
func validatePreparedProfiles(prepared preparedProfiles) error {
	expectedCommand, err := commandForClass(prepared.Command.Class)
	if err != nil || !reflect.DeepEqual(prepared.Command, expectedCommand) {
		return fmt.Errorf("prepared command profile is not an exact command class")
	}
	if prepared.TimeoutMS != nil && (*prepared.TimeoutMS < 1 || *prepared.TimeoutMS > 86_400_000) {
		return fmt.Errorf("prepared timeout_ms must be null or integer 1..86400000")
	}
	if err := validatePreparedEnvironment(prepared); err != nil {
		return err
	}
	if err := validatePreparedManifestDigest(
		"tool", toolDigestDomain, prepared.Tool, prepared.ToolSHA256,
	); err != nil {
		return err
	}
	return validatePreparedManifestDigest(
		"source", sourceDigestDomain, prepared.Source, prepared.SourceTreeSHA256,
	)
}

func validatePreparedEnvironment(prepared preparedProfiles) error {
	manifest, digest, child, err := environmentSnapshot(environmentStrings(prepared.Environment.Variables))
	if err != nil {
		return fmt.Errorf("prepared environment profile: %w", err)
	}
	if digest != prepared.EnvironmentSHA256 || !reflect.DeepEqual(manifest, prepared.Environment) {
		return fmt.Errorf("prepared environment manifest or digest was modified")
	}
	if !reflect.DeepEqual(child, prepared.ChildEnvironment) {
		return fmt.Errorf("prepared child environment does not match its manifest")
	}
	return nil
}

func validatePreparedManifestDigest(label, domain string, manifest any, expected string) error {
	_, digest, err := digestManifest(domain, manifest)
	if err != nil {
		return fmt.Errorf("prepared %s manifest: %w", label, err)
	}
	if digest != expected {
		return fmt.Errorf("prepared %s manifest does not match its digest", label)
	}
	return nil
}
