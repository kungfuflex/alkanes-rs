# pkg.alkanes.build — the npm registry proxy

The JavaScript proxy that used to live here (`server.js` + `nginx.conf`) has been
**replaced by a tlsd wasip2 module** and is no longer part of this repo.

## Where it lives now

- **Source:** [`pyrosec/tlsfetch`](https://github.com/pyrosec/tlsfetch) →
  `crates/tlsd-npm-ar-proxy` (a `wasi:http/proxy` Tier-2 module served by `tlsd`).
- **Deploy:** Google Cloud Run service `npm-proxy` (project `pkg-alkanes-build`,
  region `us-central1`), fronted by CloudFlare as `pkg.alkanes.build`. Build +
  cutover via `crates/tlsd-npm-ar-proxy/cloudrun/build.sh`.

## Why (issue #286)

The old proxy served mutable `?v=` tags with caching that let warm pnpm/CI caches
serve a **stale artifact** forever under an immutable-looking label. The module
fixes this on both sides:

- **Proxy side:** a request that pins a concrete version
  (`/dist/<pkg>?v=<version>`) is served `Cache-Control: immutable`; a request that
  resolves a moving dist-tag gets `max-age=60, must-revalidate`.
- **Publish side:** `.github/workflows/publish-npm.yml` publishes build-unique
  version tags (`<base>-<sha>.<run>`) and emits a content-addressed (sha256) build
  manifest, so consumers pin bytes, not a moving tag.

## Routes (drop-in parity with the old proxy)

| route | behavior |
|---|---|
| `/` | styled autoindex of published packages |
| `/health` | liveness |
| `/dist/<pkg>?v=<version>` | tarball; immutable when pinned (#286) |
| `/versions/<pkg>` | JSON version list + dist-tags |
| `/<@scope/pkg>` | npm-view-style metadata summary + install hints |
| everything else | passthrough to the AR npm endpoint (registry tarballs, npm API) |

Install:

```sh
pnpm install --save-dev 'https://pkg.alkanes.build/dist/@alkanes/ts-sdk?v=<version>'
```
