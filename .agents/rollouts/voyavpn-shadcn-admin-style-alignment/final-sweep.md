# Final Sweep

Batch: `06-02-global-guard-sweep`
Date: 2026-06-05

## Guard Results

- `test -f .agents/rollouts/voyavpn-shadcn-admin-style-alignment/final-sweep.md`
  - Result: passes after this file was created.
- `bash -lc 'if rg -n "data-accent|accent-primary|:root\[data-accent|setAccent|type Accent|bg-teal-600|bg-sky-600|bg-rose-600" src; then exit 1; fi'`
  - Result: passes; no old brand accent selectors, setters, classes, or tests found in `src`.
- `bash -lc 'if rg -n "forwardRef" src/components/ui; then exit 1; fi'`
  - Result: passes; no `forwardRef` remains in `src/components/ui`.
- `pnpm typecheck`
  - Result: passes; `tsc -b --pretty false` completed successfully.
- Additional sanity scan: `rg -n "data-accent|accent-primary|:root\[data-accent|setAccent|type Accent|teal|sky|rose" src`
  - Result: passes; no broader teal, sky, or rose accent remnants found in `src`.

## Raw Control Sweep

Command:

```sh
rg -n "<(input|select|checkbox)\b|type=\"(checkbox|file|radio)\"|<textarea\b" src
```

Hits:

- `src/components/ui/input.tsx`
  - Intentional shared primitive implementation. The raw `<input>` is wrapped by the aligned `Input` component with `data-slot`, shadcn-admin focus styling, disabled state, invalid state, and tokenized border/background classes.
- `src/components/ui/textarea.tsx`
  - Intentional shared primitive implementation. The raw `<textarea>` is wrapped by the aligned `Textarea` component with `data-slot`, shadcn-admin focus styling, disabled state, invalid state, and tokenized border/background classes.
- `src/features/subscriptions/import-profiles-dialog.tsx`
  - Intentional hidden file input at `#import-payload-file`. It is `sr-only`, triggered by a styled `Button`/`Label`, and preserves the file-upload behavior while keeping the visible control on shared primitives.
- `src/features/qr/qr-dialog.tsx`
  - Intentional hidden file input used for image selection. It is `hidden`, triggered by a styled `Button`, and preserves QR image scanning behavior while keeping the visible control on shared primitives.

No raw `<select>`, visible raw checkbox, or visible ad hoc text input controls remain in the sweep output.
