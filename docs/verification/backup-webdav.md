# Backup And WebDAV Verification

Batch: `07-01-backup-webdav`

Implemented scope:

- Local backup archives Voya app state as a zip with a `guiConfigs/` root, including `guiNConfig.json`, `voyavpn.sqlite`, and generated config files under `guiConfigs/`.
- Local restore validates the archive root, rejects unsafe zip paths, restores JSON config, copies generated config files, and imports SQLite tables into a clean app state.
- WebDAV uses `reqwest` for HTTP and `quick-xml` for PROPFIND multistatus parsing.
- WebDAV behavior covers `PROPFIND`, `MKCOL`, `PUT`, `GET`, and `DELETE`; tests fixture HTTP responses and XML instead of using live WebDAV.
- The Backup and Restore dialog is available from Tools and calls generated IPC wrappers.

Verification commands:

```sh
cargo test -p voya-net webdav --all-targets
cargo test -p voya-app backup --all-targets
pnpm typecheck
pnpm test -- --run
test -f docs/verification/backup-webdav.md
```

External WebDAV was not run for this batch. The batch requirement explicitly avoids live WebDAV in tests, so coverage is fixture-based.
