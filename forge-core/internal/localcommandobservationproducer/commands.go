package localcommandobservationproducer

import "fmt"

const (
	CommandGate     = "gate"
	CommandCheck    = "check"
	CommandAccept   = "accept"
	CommandProbeAll = "probe_all"
)

// commandForClass is a closed command profile. The package never accepts an
// arbitrary argv from untrusted input.
func commandForClass(class string) (commandProfile, error) {
	var argv []string
	switch class {
	case CommandGate:
		argv = []string{"node", "harness/gate.mjs"}
	case CommandCheck:
		argv = []string{"python3", "harness/check.py", "."}
	case CommandAccept:
		argv = []string{"node", "harness/acceptance.mjs"}
	case CommandProbeAll:
		argv = []string{"node", "harness/acceptance.mjs", "--json"}
	default:
		return commandProfile{}, fmt.Errorf("unsupported local command class %q", class)
	}
	return commandProfile{Argv: argv, Class: class, EvidenceType: "gate_result"}, nil
}
