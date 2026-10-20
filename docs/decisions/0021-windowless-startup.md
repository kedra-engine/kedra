# ADR 0021: Windowless startup and a fast project-opening path

Status: accepted by owner direction.

## Decision

Supersede the initial staged splash from ADRs 0002/0006. Verify the immutable
editor delivery and load the authored editor catalog concurrently, without a
native startup window, GPU readback barrier or decorative image dependencies.
Resolve recent-project history and an optional CLI project before choosing the
first real native surface. Integrity/source errors remain terminal diagnostics;
there is no fallback UI or embedded substitute asset set.

Project preparation still validates sources on the I/O pool, retains generation
checks and only remembers successful opens. Successful work enters the editor
immediately without creating or waiting for a loading window. Unfinished work
may show a lightweight loading surface after 150 ms; failures show recovery UI
immediately. This threshold suppresses flashes, not work: it never delays either
task execution or successful completion. No project code is built or executed.

The loading surface retains truthful progress, logs, retry/back and the compact
logo, but no large release illustration. Existing native surface ownership and
pending-view retirement remain authoritative. No runtime/editor layer edge or
parallel scheduler is introduced.

## Alternatives and consequences

- Keeping the splash requires its resources and a renderer fence before useful
  work can finish. It also creates/retires an extra native window.
- A minimum loading display duration hides fast completion; it is rejected.
- Skipping integrity/source checks would be faster but violates asset authority.
- Background work before the first window has no native visual feedback; CLI/log
  diagnostics remain visible. GPU/Bevy initialization itself is not eliminated.
- First-window rendering and slow/failure recovery need native input and frames;
  headless state tests alone do not establish presentation quality.

## Verification

Cover concurrent verification/catalog readiness, changed/missing delivery data,
startup timeout and no splash window. Cover zero-time fast completion, delayed
working UI, immediate failure, retry/back, stale generations, Unicode identities
and native frame/input checks. Keep local timing samples and profiling under
`target/`, not as permanent performance promises.
