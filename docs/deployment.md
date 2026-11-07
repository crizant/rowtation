# Cloudflare Pages deployment

## Deployment model

[The GitHub workflow](../.github/workflows/cloudflare-pages.yml) builds and tests
Rowtation on Ubuntu 24.04, then uploads the generated `dist/Rowtation/` static
website to Cloudflare Pages. Pushes to `main` and manual workflow runs trigger it.
NSH is fixed at 0.4.0, while rustup supplies stable Rust and the WASM target.
No native application is packaged.

This follows Cloudflare's [Direct Upload with continuous integration](https://developers.cloudflare.com/pages/how-to/use-direct-upload-with-continuous-integration/).
The existing `rowtation` project was explicitly created on Pages.
The Pages project uses Direct Upload; builds run on GitHub, so do not configure
a second Cloudflare Git build for this project.

The intended Pages project is `rowtation` with production branch `main`.
The production URL is https://rowtation.pages.dev once the first upload succeeds.
This publishes the current counter scaffold; converter features are still planned.

## One-time configuration

The repository needs these Actions secrets and variables:

| Kind | Name | Value |
| --- | --- | --- |
| Secret | `CLOUDFLARE_ACCOUNT_ID` | Account containing the Pages project |
| Secret | `CLOUDFLARE_API_TOKEN` | Token with Account → Cloudflare Pages → Edit, scoped to that account |
| Variable | `CLOUDFLARE_PAGES_PROJECT` | `rowtation` |
| Variable | `CLOUDFLARE_PAGES_ENABLED` | `true` to enable deployment after successful builds |

Create the token in the Cloudflare dashboard and store it directly as a GitHub
repository secret. Do not commit it or paste it into chat. The CLI uses a hidden
prompt when no body argument is supplied:

```sh
gh secret set CLOUDFLARE_API_TOKEN --repo crizant/rowtation
```

The local `wrangler login` session is for interactive account setup and manual
uploads. It does not authenticate GitHub Actions. Use the dedicated API token
above for unattended deployment.

Once the project and both secrets are ready, enable deployment and run it:

```sh
gh variable set CLOUDFLARE_PAGES_ENABLED --body true --repo crizant/rowtation
gh workflow run cloudflare-pages.yml --ref main --repo crizant/rowtation
```

Until the enabled variable is `true`, pushes build and test the website but skip
the deployment job. A successful build alone does not mean the site is deployed.

## Validation and troubleshooting

The build job runs formatting, Rust/TSX checking, bundling, headless tests and
browser tests in Chrome before packaging. NSH's installer verifies the release
archive checksum. Rust dependencies and compiled outputs are cached by compiler
and Cargo manifests. Deployments run serially and use the successful build's
artifact; the upload job does not rebuild it.

View runs in [GitHub Actions](https://github.com/crizant/rowtation/actions/workflows/cloudflare-pages.yml).
A skipped deploy means the enabled variable is absent or false. An authorization
failure usually means the token expired, has insufficient Pages permissions or
is scoped to another account. Verify the project variable if Wrangler cannot find
it. A failed build does not deploy or replace the current site.

To pause automatic uploads, set `CLOUDFLARE_PAGES_ENABLED` to `false`. Builds will
continue. Manage domains and restore a previous production deployment in the
Cloudflare Pages dashboard when needed.
