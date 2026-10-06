#!/usr/bin/env bash
# Publish committed HEAD; unrelated working-tree changes are excluded.
set -euo pipefail
ROOT="$(git -C "$(dirname "$0")/../../.." rev-parse --show-toplevel)"
COMMIT="$(git -C "$ROOT" rev-parse HEAD)"
: "${LOCUST_DEPLOY_SERVER:?Set LOCUST_DEPLOY_SERVER to the deployment SSH destination}"
SERVER="$LOCUST_DEPLOY_SERVER"
REMOTE=/var/www/locust.farm
SSH_ARGS=(-o BatchMode=yes -o StrictHostKeyChecking=yes -o ForwardAgent=no)
SCRATCH="$(mktemp -d)"
trap 'rm -rf "$SCRATCH"' EXIT

git clone --quiet --shared --no-checkout "$ROOT" "$SCRATCH/source"
git -C "$SCRATCH/source" checkout --quiet --detach "$COMMIT"
cd "$SCRATCH/source/sites/locust.farm"
[[ "$(node --version)" == "v$(cat .node-version)" ]] || {
    echo "Use the Node version in .node-version" >&2
    exit 1
}
npm ci
npm run lint
npm run check
npm test
npm run build
node --input-type=module <<'JS'
import { readFileSync, readdirSync, writeFileSync } from 'node:fs';
import { createHash } from 'node:crypto';
const files = readdirSync('build', { recursive: true, withFileTypes: true })
    .filter((entry) => entry.isFile())
    .map((entry) => `${entry.parentPath}/${entry.name}`.replace(/^build\//, ''))
    .sort();
writeFileSync('build/SHA256SUMS', files.map((file) =>
    `${createHash('sha256').update(readFileSync(`build/${file}`)).digest('hex')}  ${file}\n`
).join(''));
JS
ssh "${SSH_ARGS[@]}" "$SERVER" "set -e; test -r /etc/letsencrypt/live/locust.farm/fullchain.pem; test -L /etc/nginx/sites-enabled/locust.farm; mkdir -p '$REMOTE/releases/$COMMIT.part'"
rsync -az --delete -e "ssh ${SSH_ARGS[*]}" build/ "$SERVER:$REMOTE/releases/$COMMIT.part/"
ssh "${SSH_ARGS[@]}" "$SERVER" bash -s -- "$COMMIT" <<'REMOTE'
set -euo pipefail
commit="$1"
root=/var/www/locust.farm
cd "$root/releases/$commit.part"
sha256sum --quiet --check SHA256SUMS
chmod -R a+rX .
if [[ -d "$root/releases/$commit" ]]; then
    cmp SHA256SUMS "$root/releases/$commit/SHA256SUMS"
    rm -rf "$root/releases/$commit.part"
else
    mv "$root/releases/$commit.part" "$root/releases/$commit"
fi
ln -s "releases/$commit" "$root/current.next"
mv -Tf "$root/current.next" "$root/current"
REMOTE

for path in / /start /docs /docs/next/index.json /llms.txt; do
    code="$(curl --silent --show-error --output /dev/null --write-out '%{http_code}' "https://locust.farm$path")"
    [[ "$code" == 200 ]] || { echo "Expected public access at $path, got $code" >&2; exit 1; }
done
echo "Deployed $COMMIT to https://locust.farm (public HTTPS)."
