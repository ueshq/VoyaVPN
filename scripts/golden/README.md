# Golden Export Helpers

This directory is for helper material used to export reference JSON from the read-only v2rayN checkout.

Reference repo:

```text
/Users/afu/Dev/refs/v2rayN/v2rayN
```

Do not edit that checkout for normal VoyaVPN development. If a C# export harness is needed, copy `VoyaGoldenExportHarness.cs.example` into a temporary copy of `ServiceLib.Tests`, run it there, and copy only reviewed JSON outputs back into `tests/golden`.

Rust-side canonical verification is in `crates/voya-core/src/golden.rs` and is run with:

```sh
cargo test -p voya-core golden --all-targets
```
