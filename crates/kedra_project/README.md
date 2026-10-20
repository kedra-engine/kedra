# kedra_project

Authoring-layer project identity, source manifests, read-only validation and safe
creation. This crate has no Bevy, renderer or editor dependency. Public APIs expose
owned paths/manifests and actionable errors; opening never saves or executes code.

`create` owns only its newly created directory and publishes the manifest last.
`validate_creation` is a preflight, not a reservation. `recent` owns bounded local
preferences separately from project sources. See [startup](../../docs/startup.md)
for source-format, failure, concurrency and durability limits.
Project manifests and history inputs must be regular files; static FIFOs and other
special files fail before opening. Symlinks to regular files remain supported.
Portable type preflight is not a sandbox against concurrent path replacement.

```sh
cargo test -p kedra_project --locked
```
