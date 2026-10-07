# Marketing Site

`https://voyavpn.wangc.ai` is the product site, and it holds the two URLs the
App Store asks for:

| App Store Connect field | URL |
| --- | --- |
| Support URL | `https://voyavpn.wangc.ai/support` |
| Privacy Policy URL | `https://voyavpn.wangc.ai/privacy` |
| Marketing URL | `https://voyavpn.wangc.ai/` |

The site lives in `apps/web`. React renders every page to static HTML at build
time and Cloudflare serves the files as a static-assets Worker, so the pages
run no script, set no cookie, and have no analytics. English is at the root,
and the Chinese translations are under `/zh-hans/` and `/zh-hant/`.

## Keeping the privacy page true

The privacy policy has to state the same facts as the VPN answers in
[app-store-review-notes.md](app-store-review-notes.md). Change both in the same
commit, in all three languages (`apps/web/src/content/*.ts`). The test
`apps/web/test/privacy-sync.test.ts` fails if the policy stops naming a host
that the VPN answers name. Bump `PRIVACY_EFFECTIVE_DATE` in
`apps/web/src/site.ts` when the substance changes.

The screenshots in `apps/web/public/screens/` were captured from the desktop
renderer against the Tauri IPC mock at 2x, in every language and theme.
Recapture them when the Home or Nodes page changes visibly.

## Build and preview

```sh
vp run --filter @voya/web build   # writes apps/web/dist
vp run --filter @voya/web dev     # build, then serve dist with wrangler dev
```

`wrangler dev` applies the same asset rules as production:

- `/support` serves `support.html`, and `/support.html` redirects to it.
- Unknown paths get the nearest `404.html`.
- `public/_headers` sets the CSP and cache headers.

## Deploy

wrangler is not a workspace dependency; the scripts run it through `npx`. The
`wangc.ai` zone is in the same Cloudflare account, so the custom domain in
`apps/web/wrangler.jsonc` makes wrangler create the DNS record and the
certificate.

```sh
npx wrangler@4 whoami                  # confirm the account that owns wangc.ai
vp run --filter @voya/web deploy     # build, then wrangler deploy
```

Keep `run`: a bare `pnpm deploy` would be pnpm's built-in command, not this
script.
`workers_dev` stays `false`, because `*.workers.dev` is unreachable from
mainland China.

Verify:

```sh
curl -sI https://voyavpn.wangc.ai/support   # 200, content-security-policy set
curl -sI https://voyavpn.wangc.ai/privacy   # 200
curl -sI https://voyavpn.wangc.ai/nope      # 404
```

## Contact address

Both pages give `support@wangc.ai` (`SUPPORT_EMAIL` in `apps/web/src/site.ts`).
Cloudflare Email Routing on the `wangc.ai` zone has to forward it to a mailbox
someone reads; App Review may write to it.

## Download links

`DOWNLOADS` in `apps/web/src/site.ts` lists one URL per platform. A `null`
entry renders as "Coming soon". Fill each one in when its store listing or
release is live, then redeploy.
