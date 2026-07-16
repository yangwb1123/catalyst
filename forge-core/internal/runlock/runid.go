package runlock

import (
	"crypto/rand"
	"fmt"
	"time"
)

// NewRunID returns a process-scoped, roughly time-ordered, collision-
// resistant id for trace correlation: hex(time.Now().UnixNano()) + "-" +
// hex(4 crypto/rand bytes). No UUID library (forge-core's zero-dependency
// rule) — this is enough entropy to make two runs started in the same
// nanosecond tick (the only way the time component alone could collide)
// distinguishable in practice, without any package-level state to guard.
//
// Independent of Lock/Acquire — callers invoke it once per process wherever
// a Tracer is constructed (see cmd/forge's openTracer).
func NewRunID() string {
	var b [4]byte
	// crypto/rand.Read on the stdlib's Reader never returns a short read
	// without an error on any supported platform; a non-nil err here means
	// the platform's entropy source is broken, in which case the all-zero
	// fallback still yields a valid (if less unique) id rather than a panic
	// — this is a trace-correlation label, not a security token.
	_, _ = rand.Read(b[:])
	return fmt.Sprintf("%x-%x", time.Now().UnixNano(), b)
}
