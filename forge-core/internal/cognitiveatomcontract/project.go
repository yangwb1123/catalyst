package cognitiveatomcontract

import (
	"bytes"
	"fmt"
	"sort"

	"forgeos/forge-core/internal/governancecontract"
)

// Closure describes the exact minimal transitive GovernanceRecordSet closure
// used for one source claim. It is evidence for deterministic comparison only;
// it does not grant authority.
type Closure struct {
	CanonicalJSON []byte
	SHA256        string
	RecordCount   int64
	ByteCount     int64
}

// Projection is a deterministic shadow result. SourceClosures is keyed by
// source KnowledgeClaim record_id.
type Projection struct {
	Atoms                []*CognitiveAtom
	CanonicalAtomSetJSON []byte
	AtomSetSHA256        string
	SourceClosures       map[string]Closure
}

// ProjectRecordSet revalidates an exact GovernanceRecordSet v1 and projects
// its seven overlapping shadow claim types. lesson/proposal records remain
// valid closure dependencies but are not atom roots.
func ProjectRecordSet(taskID string, sourceSetJSON []byte) (*Projection, error) {
	if err := validateIdentifier("task_id", taskID); err != nil {
		return nil, err
	}
	records, err := governancecontract.DecodeRecordSet(sourceSetJSON)
	if err != nil {
		return nil, fmt.Errorf("source GovernanceRecordSet: %w", err)
	}
	byID := make(map[string]*governancecontract.Record, len(records))
	for _, record := range records {
		byID[record.Header().RecordID] = record
	}
	atoms := make([]*CognitiveAtom, 0, len(records))
	closures := make(map[string]Closure)
	for _, record := range records {
		if record.Claim == nil || !isProjectableClaimType(record.Claim.Spec.ClaimType) {
			continue
		}
		closure, err := buildClosure(record, byID)
		if err != nil {
			return nil, fmt.Errorf("claim %q closure: %w", record.Header().RecordID, err)
		}
		atom, err := projectClaim(taskID, record.Claim, closure)
		if err != nil {
			return nil, fmt.Errorf("claim %q projection: %w", record.Header().RecordID, err)
		}
		atoms = append(atoms, atom)
		closures[record.Header().RecordID] = closure
	}
	if len(atoms) == 0 {
		return nil, fmt.Errorf("source GovernanceRecordSet contains no projectable KnowledgeClaim")
	}
	sortAtoms(atoms)
	setJSON, err := CanonicalAtomSetJSON(atoms)
	if err != nil {
		return nil, err
	}
	return &Projection{
		Atoms: atoms, CanonicalAtomSetJSON: setJSON,
		AtomSetSHA256:  domainDigest(atomSetDigestDomain, setJSON),
		SourceClosures: closures,
	}, nil
}

// Project is an alias for ProjectRecordSet.
func Project(taskID string, sourceSetJSON []byte) (*Projection, error) {
	return ProjectRecordSet(taskID, sourceSetJSON)
}

// CompareProjection strictly decodes the supplied atom set, deterministically
// reprojects the source record set, and requires byte-for-byte equality.
func CompareProjection(taskID string, sourceSetJSON, candidateAtomSetJSON []byte) error {
	if _, err := DecodeAtomSet(candidateAtomSetJSON); err != nil {
		return fmt.Errorf("candidate CognitiveAtom set: %w", err)
	}
	expected, err := ProjectRecordSet(taskID, sourceSetJSON)
	if err != nil {
		return err
	}
	if !bytes.Equal(candidateAtomSetJSON, expected.CanonicalAtomSetJSON) {
		return fmt.Errorf("CognitiveAtom projection differs from deterministic re-projection")
	}
	return nil
}

func isProjectableClaimType(claimType string) bool {
	_, exists := projectableStates[claimType]
	return exists
}

func projectClaim(taskID string, claim *governancecontract.KnowledgeClaim, closure Closure) (*CognitiveAtom, error) {
	atomID, err := deriveAtomID(taskID, claim.Integrity.CanonicalSHA256, claim.Metadata.ContextSHA256, claim.Metadata.PolicySHA256, claim.Metadata.SourceTreeSHA256, claim.Metadata.SourceRevision)
	if err != nil {
		return nil, err
	}
	atom := &CognitiveAtom{
		APIVersion: APIVersion,
		Integrity:  Integrity{Canonicalization: Canonicalization},
		Kind:       Kind,
		Metadata: Metadata{
			AtomID: atomID, ContextSHA256: claim.Metadata.ContextSHA256,
			PolicySHA256: claim.Metadata.PolicySHA256, ProjectID: claim.Metadata.ProjectID,
			Scope: claim.Metadata.Scope, SourceRevision: claim.Metadata.SourceRevision,
			SourceTreeSHA256: claim.Metadata.SourceTreeSHA256, TaskID: taskID,
		},
		Source: Source{
			CanonicalSHA256:  claim.Integrity.CanonicalSHA256,
			ClaimAggregateID: claim.Metadata.AggregateID, ClaimRecordID: claim.Metadata.RecordID,
			ClaimSequence: claim.Metadata.Sequence, ClosureByteCount: closure.ByteCount,
			ClosureRecordCount: closure.RecordCount, ClosureSHA256: closure.SHA256,
			RecordKind: governancecontract.ClaimKind,
		},
		Spec: Spec{
			AtomType: claim.Spec.ClaimType, AuthorityRef: nil,
			ContradictingEvidenceRecordIDs: cloneStrings(claim.Spec.ContradictingEvidenceRecordIDs),
			DerivedFromClaimRecordIDs:      cloneStrings(claim.Spec.DerivedFromClaimRecordIDs),
			EpistemicState:                 claim.Status.State, Hardness: "none", InstructionAllowed: false,
			ProjectionConfidenceMicros: cloneInt64(claim.Spec.ConfidenceMicros), ProjectionMode: "shadow",
			Proposition: Proposition{
				ObjectType: claim.Spec.ObjectType, ObjectValue: claim.Spec.ObjectValue,
				Predicate: claim.Spec.Predicate, Subject: claim.Spec.Subject,
			},
			SupportingEvidenceRecordIDs: cloneStrings(claim.Spec.SupportingEvidenceRecordIDs),
			Validity: Validity{
				ValidFromUnixMS:  claim.Status.ValidFromUnixMS,
				ValidUntilUnixMS: cloneInt64(claim.Status.ValidUntilUnixMS),
			},
		},
	}
	if err := sealAtom(atom); err != nil {
		return nil, err
	}
	return atom, nil
}

func buildClosure(source *governancecontract.Record, byID map[string]*governancecontract.Record) (Closure, error) {
	ids, err := collectClosureIDs(source.Header().RecordID, byID)
	if err != nil {
		return Closure{}, err
	}
	encoded, err := encodeClosure(ids, byID)
	if err != nil {
		return Closure{}, err
	}
	return Closure{
		CanonicalJSON: append([]byte(nil), encoded...),
		SHA256:        domainDigest(closureDigestDomain, encoded),
		RecordCount:   int64(len(ids)), ByteCount: int64(len(encoded)),
	}, nil
}

func collectClosureIDs(sourceID string, byID map[string]*governancecontract.Record) ([]string, error) {
	seen := make(map[string]bool)
	var visit func(string) error
	visit = func(recordID string) error {
		if seen[recordID] {
			return nil
		}
		record, exists := byID[recordID]
		if !exists {
			return fmt.Errorf("missing referenced governance record %q", recordID)
		}
		seen[recordID] = true
		references := append([]string(nil), record.Header().SupersedesRecordIDs...)
		if record.Claim != nil {
			references = append(references, record.Claim.Spec.SupportingEvidenceRecordIDs...)
			references = append(references, record.Claim.Spec.ContradictingEvidenceRecordIDs...)
			references = append(references, record.Claim.Spec.DerivedFromClaimRecordIDs...)
		}
		for _, reference := range references {
			if err := visit(reference); err != nil {
				return err
			}
		}
		return nil
	}
	if err := visit(sourceID); err != nil {
		return nil, err
	}
	ids := make([]string, 0, len(seen))
	for recordID := range seen {
		ids = append(ids, recordID)
	}
	sort.Strings(ids)
	return ids, nil
}

func encodeClosure(ids []string, byID map[string]*governancecontract.Record) ([]byte, error) {
	buffer := bytes.NewBuffer(make([]byte, 0, 2+len(ids)*256))
	buffer.WriteByte('[')
	for index, recordID := range ids {
		if index > 0 {
			buffer.WriteByte(',')
		}
		recordJSON := byID[recordID].RecordJSON()
		if recordJSON == nil {
			return nil, fmt.Errorf("record %q no longer validates", recordID)
		}
		buffer.Write(recordJSON)
	}
	buffer.WriteByte(']')
	if buffer.Len() > maxSetBytes {
		return nil, fmt.Errorf("closure exceeds %d bytes", maxSetBytes)
	}
	return buffer.Bytes(), nil
}

func cloneStrings(values []string) []string {
	result := make([]string, len(values))
	copy(result, values)
	return result
}

func cloneInt64(value *int64) *int64 {
	if value == nil {
		return nil
	}
	copyValue := *value
	return &copyValue
}
