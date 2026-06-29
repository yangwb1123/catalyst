// Package risk derives a routing RISK LEVEL (low|medium|high|critical) from the
// declared characteristics of a change. It is the missing INPUT SOURCE for the
// router's hard floor: routing.TierForScore already pins risk == "critical" to
// Opus (safety_override), but nothing computed that level — the hard rule sat
// "waiting for an input with no producer." This package is that producer.
//
// It is a faithful, runnable distillation of .agent/routing/policy.yml's `risk`
// dimension (signals: blast_radius, reversibility, prod_traffic) joined with the
// `security` dimension's touches_payment / touches_auth / touches_secrets, mapped
// to the four levels the policy declares (levels: [low, medium, high, critical]),
// where `critical` is the level that trips safety_override.
//
// HONESTY — what this is and is NOT (do not oversell it):
//   - This is a RULE-BASED classifier: it maps DECLARED feature flags to a level
//     via fixed, auditable thresholds. It does not learn, infer, or guess.
//   - The features themselves are taken as EXPLICIT INPUT (supplied by the
//     orchestrator / CLU). v1 deliberately does NOT auto-extract them.
//   - AUTOMATIC feature extraction — parsing a git diff to decide which files
//     changed, whether a payment/auth code path was touched, the real blast
//     radius — is downstream wiring and is OUT OF SCOPE here = a later pass / v3.
//     When it lands, it feeds THIS function; the level mapping below is the
//     stable contract it will target.
//
// Pure: no I/O, no globals, deterministic. Fully unit-testable.
package risk

import (
	"sort"
	"strings"
)

// Level names, ascending in severity. `Critical` is the one safety_override
// keys on (routing.TierForScore forces Opus when risk == Critical).
const (
	Low      = "low"
	Medium   = "medium"
	High     = "high"
	Critical = "critical"
)

// largeBlastRadius is the module/file count at or above which a change's reach is
// "large" for escalation purposes. policy.yml lists blast_radius as a risk signal
// but fixes no number (the real scorer normalizes it); v1 picks a small, honest
// threshold — touching >= this many modules is broad enough that, combined with
// irreversibility, it warrants critical, and on its own warrants at least medium.
const largeBlastRadius = 5

// mediumBlastRadius is the lower band: at/above this a change is no longer a
// trivial one-spot edit and floors to at least medium.
const mediumBlastRadius = 2

// Signals are the DECLARED characteristics of a change, supplied explicitly by
// the caller (orchestrator/CLI). BlastRadius is the count of affected modules or
// files; the booleans mirror policy.yml's risk + security signal names.
type Signals struct {
	TouchesPayment   bool // change reaches billing/payment code (security: touches_payment)
	TouchesAuth      bool // change reaches authn/authz code (security: touches_auth)
	TouchesSecrets   bool // change reaches secrets/credentials (security: touches_secrets)
	TouchesMigration bool // change includes a schema/data migration
	ProdTraffic      bool // change is exercised by production traffic (risk: prod_traffic)
	Reversible       bool // change can be cleanly rolled back (risk: reversibility)
	BlastRadius      int  // affected modules/files count (risk: blast_radius)
}

// Classify maps declared Signals to a routing risk level and a human-readable
// reason naming the deciding factors. Deterministic and pure.
//
// Mapping (policy.yml semantics; highest applicable level wins):
//
//	critical : an IRREVERSIBLE change that ALSO hits payment, OR a large blast
//	           radius, OR production traffic, OR a production migration — the
//	           cost of getting it wrong with no rollback dominates any saving.
//	           (This is the level that trips safety_override -> forces Opus.)
//	high     : touches payment / auth / secrets (the security floor), OR a
//	           migration that runs against production traffic.
//	medium   : a reversible-but-nontrivial change — a migration, production
//	           traffic, or a non-trivial blast radius — none individually grave.
//	low      : an ordinary, small, reversible change with no sensitive surface
//	           (and the empty/zero Signals value).
func Classify(s Signals) (level, reason string) {
	if level, reason := criticalReason(s); level == Critical {
		return Critical, reason
	}
	if level, reason := highReason(s); level == High {
		return High, reason
	}
	if level, reason := mediumReason(s); level == Medium {
		return Medium, reason
	}
	return Low, "low: ordinary small reversible change, no sensitive surface"
}

// criticalReason returns Critical with its factors when an irreversible change
// also carries a grave signal, or when a migration runs against production.
func criticalReason(s Signals) (string, string) {
	if !s.Reversible {
		var why []string
		if s.TouchesPayment {
			why = append(why, "touches payment")
		}
		if s.BlastRadius >= largeBlastRadius {
			why = append(why, "large blast radius")
		}
		if s.ProdTraffic {
			// A migration against production is its OWN grave factor (the doc's
			// "production migration" critical trigger); otherwise it is plain
			// production traffic. Folding it in here makes the migration-specific
			// reason reachable — the standalone branch that followed required the
			// same (ProdTraffic && !Reversible) the ProdTraffic append already
			// catches above, so it was dead code that never produced its reason.
			if s.TouchesMigration {
				why = append(why, "production migration")
			} else {
				why = append(why, "production traffic")
			}
		}
		if len(why) > 0 {
			return Critical, label(Critical, "irreversible + "+strings.Join(why, " + "))
		}
	}
	return "", ""
}

// highReason returns High when a sensitive surface is touched (the security
// floor), or when a migration runs against production traffic.
func highReason(s Signals) (string, string) {
	if sec := sensitiveSurfaces(s); len(sec) > 0 {
		return High, label(High, "touches "+strings.Join(sec, " + "))
	}
	if s.TouchesMigration && s.ProdTraffic {
		return High, label(High, "production migration")
	}
	return "", ""
}

// mediumReason returns Medium for a non-grave but non-trivial change: a
// migration, production traffic, or a non-trivial blast radius.
func mediumReason(s Signals) (string, string) {
	var why []string
	if s.TouchesMigration {
		why = append(why, "migration")
	}
	if s.ProdTraffic {
		why = append(why, "production traffic")
	}
	if s.BlastRadius >= mediumBlastRadius {
		why = append(why, "non-trivial blast radius")
	}
	if len(why) > 0 {
		return Medium, label(Medium, strings.Join(why, " + "))
	}
	return "", ""
}

// sensitiveSurfaces lists which security surfaces a change touches, in a stable
// order, so the reason string is deterministic.
func sensitiveSurfaces(s Signals) []string {
	var out []string
	if s.TouchesPayment {
		out = append(out, "payment")
	}
	if s.TouchesAuth {
		out = append(out, "auth")
	}
	if s.TouchesSecrets {
		out = append(out, "secrets")
	}
	sort.Strings(out)
	return out
}

// label formats "<level>: <why>" for the reason string.
func label(level, why string) string {
	return level + ": " + why
}

// Rank orders levels by severity so callers can compare a classifier result
// against an explicitly-supplied level (e.g. take the HIGHER of the two).
func Rank(level string) int {
	switch level {
	case Critical:
		return 3
	case High:
		return 2
	case Medium:
		return 1
	default:
		return 0
	}
}

// Higher returns the more severe of two levels (ties return a). Used to combine
// a manual --risk override with the classifier's verdict: an override may RAISE
// the level but never silently lower a risk the signals say is higher.
func Higher(a, b string) string {
	if Rank(b) > Rank(a) {
		return b
	}
	return a
}
