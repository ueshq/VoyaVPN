# IPC Binding Drift

The checked-in `src/ipc/bindings.ts` file is generated from Rust command and
event definitions with `specta` and `tauri-specta`.

Regenerate bindings:

```sh
pnpm bindings
```

Drift check:

```sh
pnpm bindings:check
```

Latest local evidence:

```text
$ pnpm bindings:check
$ node scripts/bindings.mjs --check
   Compiling voyavpn v0.1.0 (/Users/afu/Dev/refs/VoyaVPN/src-tauri)
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 8.54s
     Running `target/debug/export-bindings /var/folders/.../voyavpn-bindings-.../bindings.ts`
Generated IPC bindings are up to date.
```
