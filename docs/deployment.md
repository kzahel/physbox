# Deployment

Production is a Cloudflare Worker named `physbox5`, an R2 bucket named
`physbox5`, and custom domain `physbox5.kzahel.com`.

Install the tracked hook once with `npm run hooks:install`. On a successful
push that updates `refs/heads/main`, the hook starts a detached scheduler. It
waits until the pushed SHA is readable from the remote, settles quick
successive pushes, checks out that exact SHA into a clean sibling worktree,
builds the release web bundle, uploads assets, and deploys the Worker. Logs and
state stay under `.git/physbox5-deploy/`.

This design matters: a push never deploys uncommitted files or a newer local
commit than the SHA that actually reached the remote. Failed pushes do not
deploy. Another push supersedes an older pending deployment before upload.

Commands:

```sh
npm run bundle:web
npm run deploy:dry-run
npm run deploy
```

`deploy:dry-run` builds and validates Wrangler without uploading R2 objects or
publishing the Worker. `deploy` is an explicit manual deployment of the current
checkout; the normal production path is the exact-SHA hook.
