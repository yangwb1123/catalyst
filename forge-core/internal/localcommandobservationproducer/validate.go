package localcommandobservationproducer

import (
	"fmt"
	"reflect"
	"strings"

	commandcontract "forgeos/forge-core/internal/commandobservationevidencecontract"
)

func validateProductionPackage(value ProductionPackage) error {
	if value.APIVersion != ProductionAPIVersion || value.Canonicalization != Canonicalization {
		return fmt.Errorf("production API or canonicalization is unsupported")
	}
	if err := commandcontract.ValidateObservation(value.Observation); err != nil {
		return fmt.Errorf("production observation: %w", err)
	}
	command, err := productionCommand(value.Observation)
	if err != nil {
		return err
	}
	if err := validateProductionProducer(value.Observation); err != nil {
		return err
	}
	if err := validateEnvironmentBinding(value.EnvironmentManifest, value.Observation); err != nil {
		return err
	}
	if err := validateToolBinding(value.ToolManifest, command, value.Observation); err != nil {
		return err
	}
	return validateSourceBinding(value.SourceManifest, value.Observation)
}

func productionCommand(observation commandcontract.Observation) (commandProfile, error) {
	if observation.EvidenceType != "gate_result" || observation.Command.CWD != "." ||
		observation.Command.StdinBytes != 0 || observation.Command.StdinSHA256 != emptySHA256 {
		return commandProfile{}, fmt.Errorf("production command fixed fields drifted")
	}
	for _, class := range []string{CommandGate, CommandCheck, CommandAccept, CommandProbeAll} {
		profile, _ := commandForClass(class)
		if reflect.DeepEqual(observation.Command.Argv, profile.Argv) {
			return profile, nil
		}
	}
	return commandProfile{}, fmt.Errorf("production command argv is not a closed command class")
}

func validateProductionProducer(observation commandcontract.Observation) error {
	producer := observation.Producer
	if producer.ProducerID != ProducerID || producer.ProducerType != "tool" ||
		producer.ProducerVersion != ProducerVersion {
		return fmt.Errorf("production producer identity drifted")
	}
	return nil
}

func validateEnvironmentBinding(
	manifest EnvironmentManifest,
	observation commandcontract.Observation,
) error {
	rebuilt, digest, _, err := environmentSnapshot(environmentStrings(manifest.Variables))
	if err != nil {
		return fmt.Errorf("production environment manifest: %w", err)
	}
	if !reflect.DeepEqual(rebuilt, manifest) || digest != observation.Command.EnvironmentSHA256 {
		return fmt.Errorf("production environment manifest binding mismatch")
	}
	return nil
}

func validDigest(value string) bool {
	return len(value) == 64 && lowerHex(value)
}

func validSourceRevision(value string) bool {
	if strings.HasPrefix(value, "git-sha1:") {
		return len(value) == len("git-sha1:")+40 && lowerHex(strings.TrimPrefix(value, "git-sha1:"))
	}
	if strings.HasPrefix(value, "git-sha256:") {
		return len(value) == len("git-sha256:")+64 && lowerHex(strings.TrimPrefix(value, "git-sha256:"))
	}
	return false
}
