# Formal candidate binding

This package binds one formal acceptance run to one observable source candidate.
The coordinator fingerprints before and after live probes while an independent
raw-inotify helper permanently records ordinary path mutations, including
write-and-restore changes and ancestor-directory rebinding. Barriers linearize
when the helper has drained its inotify queue to `EAGAIN`; closing the helper is
resource cleanup and does not extend the accepted interval.

The proof boundary is deliberately narrow. It requires the initial Linux user
namespace, trusted descriptor-pinned system executables, and an allowlisted
local filesystem. Non-initial user namespaces, remote/unknown filesystems,
helper failure, queue overflow, hardlinked source files, or lost watch coverage
fail closed. Linux inotify does not prove against adversarial `mmap` writes or
privileged mount manipulation, so operators must exclude those from the formal
run's trust boundary.
