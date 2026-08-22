package decisioncapsulecontract_test

import (
	"testing"

	dc "forgeos/forge-core/internal/decisioncapsulecontract"
	kd "forgeos/forge-core/internal/kerneldecisioncontract"
	op "forgeos/forge-core/internal/kerneloperationalcontract"
)

var (
	_ func(any) ([]byte, error) = dc.CanonicalJSON

	_ func(*dc.StructuralReplayManifest, *kd.KernelDecisionReferenceClosure) (string, error)                       = dc.StructuralReplayManifestDigest
	_ func(*dc.StructuralReplayManifest, *kd.KernelDecisionReferenceClosure) error                                 = dc.ValidateStructuralReplayManifest
	_ func(*dc.StructuralReplayManifest, *kd.KernelDecisionReferenceClosure) (*dc.StructuralReplayManifest, error) = dc.SealStructuralReplayManifest
	_ func(*kd.KernelDecisionReferenceClosure) (*dc.StructuralReplayManifest, error)                               = dc.DeriveStructuralReplayManifest
	_ func([]byte, *kd.KernelDecisionReferenceClosure) (*dc.StructuralReplayManifest, error)                       = dc.DecodeStructuralReplayManifest

	_ func(*dc.DecisionCapsule) (string, error)                             = dc.DecisionCapsuleDigest
	_ func(*dc.DecisionCapsule) error                                       = dc.ValidateDecisionCapsule
	_ func(*dc.DecisionCapsule) (*dc.DecisionCapsule, error)                = dc.SealDecisionCapsule
	_ func(*kd.KernelDecisionReferenceClosure) (*dc.DecisionCapsule, error) = dc.DeriveDecisionCapsule
	_ func([]byte) (*dc.DecisionCapsule, error)                             = dc.DecodeDecisionCapsule

	_ func(*dc.EvaluationBranch, *dc.DecisionCapsule) (string, error)               = dc.EvaluationBranchDigest
	_ func(*dc.EvaluationBranch, *dc.DecisionCapsule) error                         = dc.ValidateEvaluationBranch
	_ func(*dc.EvaluationBranch, *dc.DecisionCapsule) (*dc.EvaluationBranch, error) = dc.SealEvaluationBranch
	_ func(*dc.DecisionCapsule) (*dc.EvaluationBranch, error)                       = dc.DeriveEvaluationBranch
	_ func([]byte, *dc.DecisionCapsule) (*dc.EvaluationBranch, error)               = dc.DecodeEvaluationBranch

	_ func(*dc.StructuralReplayClosure) (string, error)                                = dc.StructuralReplayClosureDigest
	_ func(*dc.StructuralReplayClosure) error                                          = dc.ValidateStructuralReplayClosure
	_ func(*dc.StructuralReplayClosure) (*dc.StructuralReplayClosure, error)           = dc.SealStructuralReplayClosure
	_ func(*dc.DecisionCapsule, []op.ArtifactRef) (*dc.StructuralReplayClosure, error) = dc.DeriveStructuralReplayClosure
	_ func([]byte) (*dc.StructuralReplayClosure, error)                                = dc.DecodeStructuralReplayClosure
)

func TestPublicWireModelsAreExternallyConstructible(t *testing.T) {
	attestations := dc.ReplayAttestations{}
	closureRef := dc.ClosureRef{}
	transactionRef := dc.DecisionTransactionRef{}
	manifestRef := dc.ManifestRef{}
	capsuleRef := dc.CapsuleRef{}
	manifest := dc.StructuralReplayManifest{Attestations: attestations,
		DecisionClosureRef: closureRef, DecisionTransactionRef: transactionRef,
		OperationalClosureRef: closureRef}
	capsule := dc.DecisionCapsule{Attestations: attestations, ReplayManifest: manifest}
	branch := dc.EvaluationBranch{Attestations: attestations, CapsuleRef: capsuleRef,
		DecisionClosureRef: closureRef, ManifestRef: manifestRef}
	outer := dc.StructuralReplayClosure{Attestations: attestations,
		DecisionCapsule: capsule, EvaluationBranch: branch}
	if _, err := dc.CanonicalJSON(&outer); err != nil {
		t.Fatal(err)
	}
}
