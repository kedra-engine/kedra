# kedra_test

This tooling crate supports native application tests in project-owned Rust scenarios.
Runtime applications opt into `kedra_app`; they do not depend on this crate.
No editor, Figma connection, Python, scenario DSL or test registry is required.
`Client::input`, `resize` and `capture` execute finite host-registered Luau
programs inside the target. The Rust scenario retains fixture setup, domain
assertions, wall-clock/process supervision and independent evidence validation.

- `cargo::build` discovers selected bin/example executables through maintained
  Cargo metadata/messages and streams compiler diagnostics. `cargo::run` executes
  a project scenario target from its manifest directory.
- `process::NativeProcess` owns a fresh private log/credential directory, bounded
  startup and mandatory graceful-exit verification. Projects supply arguments,
  readiness and shutdown authorization. Drop forcibly reaps unfinished children;
  forced cleanup is never a passing lifecycle.
- `Client` uses maintained ureq with bounded loopback-only authenticated requests.
  Effect identities are submitted once, then their existing receipts are polled.
  A timeout is not rollback. Public errors exclude response bodies and credentials.
  `scenario` starts only a named target-owned program, never source/eval. A failed
  program retains its detailed receipt in the private RPC log. No retry fallback
  silently switches from Luau to direct feature invocation.
  `operation` invokes exactly the named native method, without implicit routing
  or argument conversion. Script-backed native actions use their explicit methods.
- `evidence` checks decoded original PNGs, physical dimensions and three stable
  consecutive frames. Uniform frames need an explicit diagnostic rationale.
  Publication checks the hash again and never overwrites an image.
- `suite::capture_twice` runs the whole selected sequence in exactly two processes,
  retaining failed cases/lifecycles and incremental public reports. Project Rust
  functions own concrete feature input and readiness.
- `visual` retains the original image/PNG/dify comparator and regressions.
  `comparison` preserves complete reference/native images, configs and raw diffs.
  `text` optionally checks structured typography and exact reviewed shape pairs;
  full-frame consumers need no authored UI or font policy.
- `report` writes offline JSON/HTML with escaped text and restricted local links.
  Callers must supply only portable public evidence, never unrestricted RPC data.

The small custom code expresses Kedra session/evidence policy, not HTTP, PNG,
Cargo, rendering or perceptual comparison. Those remain maintained libraries.
There are no QA-driver/report mocks and no self-test framework. Verification uses
real native consumers plus the retained comparator/product Rust regressions.

The project-local launcher exposes:

```sh
cargo kedra test --manifest-path PATH/Cargo.toml --package PROJECT_TEST --bin PROJECT_TEST -- SCENARIO_ARGUMENTS
```

Applications choose ordinary Cargo scenario targets. The launcher supplies neither
missing scenarios nor acceptance policy. Source intake and reference review stay
explicit; native captures never become independent references automatically.
For an existing game's `examples/test.rs`, use `--example test` instead of `--bin` and
keep `kedra_test` in dev-dependencies. No separate application/staging host, fixture
workspace or editor is required: the scenario drives the project's ordinary app.
See the [architecture decision](../../docs/decisions/0028-shared-native-test.md).
