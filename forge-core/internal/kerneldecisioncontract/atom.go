package kerneldecisioncontract

import (
	"crypto/sha256"
	"encoding/hex"
	"encoding/json"
	"fmt"
	"math"
	"sort"
)

var sourceTypes = map[string]map[string]bool{
	"artifact":              allAtomTypes(),
	"artifact_receipt":      atomTypeSet("evidence", "object", "observation"),
	"capability_invocation": atomTypeSet("actor", "operation"),
	"cognitive_atom_v1":     atomTypeSet("assumption", "constraint", "decision", "fact", "hypothesis", "inference", "unknown"),
	"evidence_record":       atomTypeSet("evidence", "observation"),
	"execution_receipt":     atomTypeSet("actor", "evidence", "observation"),
	"interaction_event":     atomTypeSet("actor", "evidence", "object", "observation", "operation"),
	"work_intent":           atomTypeSet("acceptance", "constraint", "goal", "preference", "risk", "unknown"),
}

var hardnessByType = map[string]map[string]bool{
	"acceptance": atomTypeSet("advisory", "preferred", "required"),
	"actor":      atomTypeSet("none"), "assumption": atomTypeSet("advisory", "none"),
	"constraint": atomTypeSet("advisory", "contract", "invariant", "preferred", "required"),
	"decision":   atomTypeSet("advisory", "required"), "evidence": atomTypeSet("none"),
	"fact": atomTypeSet("none"), "goal": atomTypeSet("advisory", "preferred", "required"),
	"hypothesis": atomTypeSet("advisory", "none"), "inference": atomTypeSet("advisory", "none"),
	"object": atomTypeSet("none"), "observation": atomTypeSet("none"),
	"operation": atomTypeSet("none"), "preference": atomTypeSet("advisory", "preferred"),
	"risk": atomTypeSet("advisory", "none"), "unknown": atomTypeSet("advisory", "none"),
}

func atomTypeSet(values ...string) map[string]bool {
	result := make(map[string]bool, len(values))
	for _, value := range values {
		result[value] = true
	}
	return result
}

func allAtomTypes() map[string]bool {
	return atomTypeSet("acceptance", "actor", "assumption", "constraint", "decision",
		"evidence", "fact", "goal", "hypothesis", "inference", "object", "observation",
		"operation", "preference", "risk", "unknown")
}

func validateProposition(value Proposition) error {
	if err := identifier(value.Predicate, "proposition.predicate"); err != nil {
		return err
	}
	if err := identifier(value.Subject, "proposition.subject"); err != nil {
		return err
	}
	switch value.ObjectType {
	case "artifact_ref":
		member, ok := value.ObjectValue.(string)
		if !ok {
			return fmt.Errorf("artifact_ref proposition object must be text")
		}
		return identifier(member, "proposition.object_value")
	case "string":
		member, ok := value.ObjectValue.(string)
		if !ok {
			return fmt.Errorf("string proposition object must be text")
		}
		return text(member, "proposition.object_value", maxStringBytes)
	case "boolean":
		if _, ok := value.ObjectValue.(bool); !ok {
			return fmt.Errorf("boolean proposition object has wrong type")
		}
	case "integer":
		if !signedInteger(value.ObjectValue) {
			return fmt.Errorf("integer proposition object has wrong type")
		}
	case "null":
		if value.ObjectValue != nil {
			return fmt.Errorf("null proposition object must be null")
		}
	default:
		return fmt.Errorf("proposition.object_type is unsupported")
	}
	return nil
}

func signedInteger(value any) bool {
	switch number := value.(type) {
	case json.Number:
		_, err := number.Int64()
		return err == nil
	case int, int8, int16, int32, int64:
		return true
	case uint:
		return uint64(number) <= math.MaxInt64
	case uint8, uint16, uint32:
		return true
	case uint64:
		return number <= math.MaxInt64
	default:
		return false
	}
}

func validateAtomSemantics(value *CognitiveAtom) error {
	if value.AtomAPIInvalid() {
		return fmt.Errorf("CognitiveAtom constants differ")
	}
	if !allAtomTypes()[value.AtomType] {
		return fmt.Errorf("atom_type is unsupported")
	}
	if err := validateAtomMembers(value); err != nil {
		return err
	}
	return validateAtomState(value)
}

func (value *CognitiveAtom) AtomAPIInvalid() bool {
	return value.APIVersion != atomAPI || value.Canonicalization != canonicalization ||
		value.EffectiveHardness != "none" || value.InstructionAllowed || value.Kind != atomKind
}

func validateAtomMembers(value *CognitiveAtom) error {
	if err := validateAttestations(value.Attestations); err != nil {
		return err
	}
	if err := validateBindings(value.Bindings); err != nil {
		return err
	}
	if err := validateTask(value.TaskBinding); err != nil {
		return err
	}
	if err := validateProposition(value.Proposition); err != nil {
		return err
	}
	if value.Scope.Project != value.TaskBinding.ProjectID {
		return fmt.Errorf("scope.project must equal task_binding.project_id")
	}
	if err := text(value.Scope.Project, "scope.project", maxShortBytes); err != nil {
		return err
	}
	if err := validateScopeOptional(value); err != nil {
		return err
	}
	if err := validateAuthority(value.DeclaredAuthority); err != nil {
		return err
	}
	return validateSource(value.Source, value.AtomType)
}

func validateScopeOptional(value *CognitiveAtom) error {
	if value.Scope.Module != nil {
		if err := text(*value.Scope.Module, "scope.module", maxShortBytes); err != nil {
			return err
		}
	}
	if value.Scope.Object != nil {
		if err := text(*value.Scope.Object, "scope.object", maxShortBytes); err != nil {
			return err
		}
		if *value.Scope.Object != value.Proposition.Subject {
			return fmt.Errorf("scope.object must equal proposition.subject")
		}
	}
	return nil
}

func validateAtomState(value *CognitiveAtom) error {
	if value.Validity.ValidFromUnixMS < 0 || value.Validity.ValidUntilUnixMS != nil &&
		(*value.Validity.ValidUntilUnixMS < 0 || *value.Validity.ValidUntilUnixMS <= value.Validity.ValidFromUnixMS) {
		return fmt.Errorf("CognitiveAtom validity is invalid")
	}
	requiresConfidence := oneOf(value.AtomType, "assumption", "hypothesis", "inference")
	validConfidence := value.ConfidenceMicros != nil && *value.ConfidenceMicros >= 0 && *value.ConfidenceMicros <= 1000000
	if requiresConfidence != validConfidence || !requiresConfidence && value.ConfidenceMicros != nil {
		return fmt.Errorf("confidence_micros presence does not match atom_type")
	}
	if err := validateHardness(value); err != nil {
		return err
	}
	return validateEpistemic(value)
}

func validateHardness(value *CognitiveAtom) error {
	legacy := value.Source.SourceKind == "cognitive_atom_v1"
	if legacy {
		if value.DeclaredHardness != "none" || value.DeclaredAuthority.AuthorityKind != "none" {
			return fmt.Errorf("legacy source requires none hardness and authority")
		}
		return nil
	}
	if !hardnessByType[value.AtomType][value.DeclaredHardness] {
		return fmt.Errorf("declared_hardness is not admitted by atom_type")
	}
	authority := value.DeclaredAuthority.AuthorityKind
	if value.DeclaredHardness == "none" && authority != "none" {
		return fmt.Errorf("none hardness requires none authority")
	}
	if oneOf(value.DeclaredHardness, "contract", "invariant") && authority != "contract_artifact" {
		return fmt.Errorf("contract/invariant hardness requires contract artifact")
	}
	if value.DeclaredHardness == "required" && authority == "none" {
		return fmt.Errorf("required hardness requires declared authority")
	}
	if value.DeclaredHardness == "required" && value.AtomType == "decision" &&
		!oneOf(authority, "approval_record", "architecture_decision") {
		return fmt.Errorf("required decision requires ADR or Approval")
	}
	return nil
}

func validateEpistemic(value *CognitiveAtom) error {
	if value.Source.SourceKind != "cognitive_atom_v1" {
		if value.EpistemicState != "declared" {
			return fmt.Errorf("nonlegacy epistemic_state must be declared")
		}
		return nil
	}
	allowed := map[string][]string{"assumption": {"open", "testing"}, "constraint": {"candidate"},
		"decision": {"proposed"}, "fact": {"candidate", "contested"},
		"hypothesis": {"open", "testing"}, "inference": {"candidate"},
		"unknown": {"investigating", "open"}}
	if !oneOf(value.EpistemicState, allowed[value.AtomType]...) {
		return fmt.Errorf("legacy epistemic_state is outside ADR-0047 states")
	}
	return nil
}

func atomDigest(value *CognitiveAtom) (string, error) {
	blank := *value
	blank.AtomID, blank.AtomSHA256 = "", ""
	if err := validateAtomBody(&blank, true); err != nil {
		return "", err
	}
	raw, err := canonicalBytes(&blank, maxAtomBytes)
	if err != nil {
		return "", err
	}
	digest := sha256.New()
	digest.Write(atomDomain)
	digest.Write(raw)
	return hex.EncodeToString(digest.Sum(nil)), nil
}

func validateAtomBody(value *CognitiveAtom, blank bool) error {
	if value == nil {
		return fmt.Errorf("CognitiveAtom is nil")
	}
	if err := identity(value.AtomID, value.AtomSHA256, atomPrefix, "atom", blank); err != nil {
		return err
	}
	return validateAtomSemantics(value)
}

func ValidateCognitiveAtom(value *CognitiveAtom) error {
	if err := validateAtomBody(value, false); err != nil {
		return err
	}
	digest, err := atomDigest(value)
	if err != nil {
		return err
	}
	if value.AtomSHA256 != digest {
		return fmt.Errorf("atom_sha256 does not match canonical preimage")
	}
	_, err = canonicalBytes(value, maxAtomBytes)
	return err
}

func SealCognitiveAtom(value *CognitiveAtom) (*CognitiveAtom, error) {
	if value == nil || value.AtomID != "" || value.AtomSHA256 != "" {
		return nil, fmt.Errorf("sealing CognitiveAtom requires blank identity")
	}
	sealed, err := cloneValue(value)
	if err != nil {
		return nil, err
	}
	digest, err := atomDigest(sealed)
	if err != nil {
		return nil, err
	}
	sealed.AtomID, sealed.AtomSHA256 = atomPrefix+digest, digest
	return sealed, ValidateCognitiveAtom(sealed)
}

func DecodeCognitiveAtom(raw []byte) (*CognitiveAtom, error) {
	var value CognitiveAtom
	if err := decodeTyped(raw, maxAtomBytes, &value); err != nil {
		return nil, err
	}
	return &value, ValidateCognitiveAtom(&value)
}

func sortAtoms(values []CognitiveAtom) error {
	if values == nil || len(values) == 0 || len(values) > maxAtoms {
		return fmt.Errorf("cognitive_atoms cardinality must be 1..%d", maxAtoms)
	}
	ids := make([]string, len(values))
	for index := range values {
		if err := ValidateCognitiveAtom(&values[index]); err != nil {
			return fmt.Errorf("cognitive_atoms[%d]: %w", index, err)
		}
		ids[index] = values[index].AtomID
	}
	if !sort.StringsAreSorted(ids) || duplicateStrings(ids) {
		return fmt.Errorf("cognitive_atoms must be strictly atom-id sorted and unique")
	}
	return nil
}
