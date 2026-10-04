#!/bin/sh
# Public macOS arm64 bootstrap. Installs software; harness setup stays explicit.
set -eu

origin=https://locust.farm/downloads
trust_sha256=ce02bb70439f130406ea1d7febc6ced4c273427fde0a1bbbfe6126abb1b45cd3
requirement='=anchor apple generic and certificate leaf[subject.OU] = "P2Q3P9R6AT" and identifier "farm.locust.cli"'
prefix="${HOME:?HOME must name your user directory}/.local/share/locust"
bin_dir="$HOME/.local/bin"
plan_only=0

die() { printf 'Locust install: %s\n' "$*" >&2; exit 1; }
usage() {
    printf '%s\n' 'Usage: install.sh [--plan] [--prefix /absolute/path] [--bin-dir /absolute/path]'
    printf '%s\n' 'Installs verified Locust software and a locust symlink. Does not configure clients or start a daemon.'
}
while [ "$#" -gt 0 ]; do
    case "$1" in
        --plan) plan_only=1; shift ;;
        --prefix|--bin-dir)
            [ "$#" -ge 2 ] || die "$1 needs an absolute path"
            case "$1" in --prefix) prefix=$2 ;; --bin-dir) bin_dir=$2 ;; esac
            shift 2 ;;
        --help|-h) usage; exit 0 ;;
        *) die "unknown option: $1" ;;
    esac
done
for destination in "$prefix" "$bin_dir"; do
    case "$destination" in /*) ;; *) die 'installation paths must be absolute' ;; esac
    case "$destination/" in */../*|*/./*) die 'installation paths must not contain traversal components' ;; esac
done
[ "$(uname -s)" = Darwin ] && [ "$(uname -m)" = arm64 ] || die 'this published preview supports macOS Apple Silicon only'
[ ! -L "$bin_dir" ] || die 'the selected bin directory is a symlink; select a plain directory'
if [ -e "$bin_dir" ]; then
    [ -d "$bin_dir" ] || die 'the selected bin directory is not a directory'
    [ "$(stat -f %u "$bin_dir")" = "$(id -u)" ] || die 'the selected bin directory belongs to another user'
fi
link="$bin_dir/locust"
if [ -e "$link" ] || [ -L "$link" ]; then
    [ -L "$link" ] && [ "$(readlink "$link")" = "$prefix/current/locust" ] || die "preserving existing $link; select another --bin-dir or inspect it yourself"
fi

work=$(mktemp -d "${TMPDIR:-/tmp}/locust-install.XXXXXX") || die 'cannot create a private temporary directory'
trap 'rm -rf -- "$work"' EXIT
trap 'exit 1' HUP INT TERM
fetch() {
    curl --proto '=https' --proto-redir '=https' --tlsv1.2 --fail --silent --show-error --location "$1" --output "$2"
}
json() { /usr/bin/plutil -extract "$1" raw -o - "$work/latest.json"; }
sha256() { shasum -a 256 "$1" | cut -d ' ' -f 1; }
fetch "$origin/latest.json" "$work/latest.json"
release=$(json release_id)
version=$(json version)
source=$(json source_commit)
case "$release" in ''|*[!a-zA-Z0-9.-]*) die 'release metadata has an invalid release identifier' ;; esac
case "$source" in ''|*[!0-9a-f]*) die 'release metadata has an invalid source identifier' ;; esac
[ "${#source}" -eq 40 ] || die 'release metadata has an invalid source identifier length'
[ "$(json target)" = aarch64-apple-darwin ] || die 'release metadata names an unsupported target'
[ "$(json trust_key_sha256)" = "$trust_sha256" ] || die 'publisher trust changed; review a fresh bootstrap explicitly'
base="$origin/$release"
archive="locust-$release-aarch64-apple-darwin.tar.gz"
artifact_hash() {
    wanted=$1
    index=0
    while file=$(json "artifacts.$index.file" 2>/dev/null); do
        if [ "$file" = "$wanted" ]; then
            [ "$(json "artifacts.$index.url")" = "$base/$wanted" ] || die 'artifact URL does not match the release origin'
            json "artifacts.$index.sha256"
            return
        fi
        index=$((index + 1))
    done
    die "release metadata does not name $wanted"
}
archive_sha256=$(artifact_hash "$archive")
binary_sha256=$(artifact_hash locust)
fetch "$base/$archive" "$work/release.tar.gz"
[ "$(sha256 "$work/release.tar.gz")" = "$archive_sha256" ] || die 'archive checksum differs; nothing installed'
expected=$(printf '%s\n' locust skills/locust/SKILL.md manual.tar manifest.json manifest.sig trust.pub withdrawals.json withdrawals.json.sig INSTALL.md)
[ "$(tar -tzf "$work/release.tar.gz")" = "$expected" ] || die 'archive contains unexpected members; nothing installed'

# Stream known members into new regular files; archive links cannot create paths.
bundle="$work/bundle"
mkdir -p "$bundle/skills/locust"
tar -xOf "$work/release.tar.gz" locust > "$bundle/locust"
[ "$(sha256 "$bundle/locust")" = "$binary_sha256" ] || die 'bootstrap binary checksum differs; nothing executed'
chmod 0755 "$bundle/locust"
# Apple-rooted publisher verification happens before executing downloaded code.
/usr/bin/codesign --verify --strict --requirements "$requirement" "$bundle/locust" || die 'Apple publisher signature verification failed; nothing executed'
/usr/bin/codesign --display --verbose=2 "$bundle/locust" 2> "$work/signature.txt"
grep '^Timestamp=' "$work/signature.txt" > /dev/null || die 'publisher signature has no trusted timestamp; nothing executed'
for relative in skills/locust/SKILL.md manual.tar manifest.json manifest.sig trust.pub withdrawals.json withdrawals.json.sig INSTALL.md; do
    tar -xOf "$work/release.tar.gz" "$relative" > "$bundle/$relative"
    chmod 0644 "$bundle/$relative"
done
# Fetch the publisher's current signed policy separately from the immutable bundle.
fetch "$origin/withdrawals.json" "$bundle/withdrawals.json"
fetch "$origin/withdrawals.json.sig" "$bundle/withdrawals.json.sig"
[ "$(sha256 "$bundle/trust.pub")" = "$trust_sha256" ] || die 'public signing key differs; nothing installed'
bootstrap="$bundle/locust"
"$bootstrap" --json package verify --bundle "$bundle" --trust-key "$bundle/trust.pub" --withdrawals "$bundle/withdrawals.json" > "$work/verified.json"
[ "$(/usr/bin/plutil -extract result.manifest.source_commit raw -o - "$work/verified.json")" = "$source" ] || die 'verified source differs from release metadata'
[ "$(/usr/bin/plutil -extract result.manifest.version raw -o - "$work/verified.json")" = "$version" ] || die 'verified version differs from release metadata'
[ "$(/usr/bin/plutil -extract result.manifest_sha256 raw -o - "$work/verified.json")" = "$(json manifest_sha256)" ] || die 'verified manifest differs from release metadata'
"$bootstrap" --json install plan --prefix "$prefix" --bundle "$bundle" --trust-key "$bundle/trust.pub" --withdrawals "$bundle/withdrawals.json" > "$work/plan.json"
printf 'Locust %s (%s), verified macOS arm64 preview\n' "$version" "$source"
cat "$work/plan.json"
printf '\nCLI link: %s -> %s/current/locust\n' "$link" "$prefix"
if [ "$plan_only" -eq 1 ]; then
    printf '%s\n' 'Plan only. No software, service or client configuration was changed.'
    exit 0
fi
digest=$(/usr/bin/plutil -extract result.plan_sha256 raw -o - "$work/plan.json")
"$bootstrap" --json install apply --prefix "$prefix" --bundle "$bundle" --trust-key "$bundle/trust.pub" --withdrawals "$bundle/withdrawals.json" --expect-plan "$digest"
mkdir -p "$bin_dir"
if [ ! -e "$link" ] && [ ! -L "$link" ]; then
    ln -s "$prefix/current/locust" "$link"
fi
[ -L "$link" ] && [ "$(readlink "$link")" = "$prefix/current/locust" ] || die 'software installed, but CLI link changed concurrently; inspect it before continuing'
"$prefix/current/locust" --version
printf 'Installed CLI: %s\n' "$link"
case ":${PATH:-}:" in *":$bin_dir:"*) ;; *) printf 'Add %s to PATH to run locust by name.\n' "$bin_dir" ;; esac
printf '%s\n' 'Software is installed. Run locust up --client codex --workspace /YOUR/WORKSPACE --plan to review harness setup.'
