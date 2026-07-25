#!/usr/bin/env bash
# Clones the integrated engines into ../engines/<name> as READ-ONLY
# reference material, at exactly the versions pinned in
# deploy/docker-compose.yml (single source of truth — tags are parsed,
# never repeated here). Idempotent: re-running re-pins every checkout.
#
# ../engines/ is reference only (CLAUDE.md standing rules): we read
# engine source to understand behavior; we never modify it.
set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
COMPOSE_FILE="$REPO_ROOT/deploy/docker-compose.yml"
ENGINES_DIR="$(cd "$REPO_ROOT/.." && pwd)/engines"

# Extracts the tag of an image whose repository matches $1.
image_tag() {
  local repo="$1"
  local tag
  tag=$(grep -E "image:\s*${repo}:" "$COMPOSE_FILE" | head -1 | sed -E "s|.*${repo}:([^\"' ]+).*|\1|")
  if [ -z "$tag" ]; then
    echo "ERROR: no pinned image '${repo}:' found in $COMPOSE_FILE" >&2
    exit 1
  fi
  printf '%s' "$tag"
}

# Verifies a tag exists upstream; on failure lists near matches so the
# fix is obvious (no silent fallback — protocol of this repo).
verify_tag() {
  local url="$1" tag="$2"
  if [ -z "$(git ls-remote --tags "$url" "refs/tags/${tag}")" ]; then
    echo "ERROR: tag '${tag}' not found in ${url}" >&2
    echo "Nearby tags:" >&2
    git ls-remote --tags "$url" | sed 's|.*refs/tags/||' | grep -F "$(printf '%s' "$tag" | cut -c1-8)" | head -10 >&2 || true
    exit 1
  fi
}

# Shallow-clones (or re-pins) $url at $tag into $ENGINES_DIR/$name.
pin() {
  local name="$1" url="$2" tag="$3"
  local dest="$ENGINES_DIR/$name"
  verify_tag "$url" "$tag"
  if [ -d "$dest/.git" ]; then
    echo "== $name: re-pinning to $tag"
    git -C "$dest" fetch --depth 1 origin "refs/tags/${tag}:refs/tags/${tag}" --force
    git -C "$dest" checkout -q "tags/${tag}"
  else
    echo "== $name: cloning at $tag"
    git clone --depth 1 --branch "$tag" "$url" "$dest"
  fi
  echo "   $name @ $(git -C "$dest" describe --tags --always)"
}

# Collabora CODE image tags (a.b.c.d.e) correspond to CollaboraOnline
# release tags cp-a.b.c-d; verify_tag catches the mapping ever drifting.
collabora_repo_tag() {
  local image_tag="$1"
  local a b c d
  IFS='.' read -r a b c d _ <<<"$image_tag"
  printf 'cp-%s.%s.%s-%s' "$a" "$b" "$c" "$d"
}

mkdir -p "$ENGINES_DIR"

SYNAPSE_TAG="$(image_tag 'ghcr\.io/element-hq/synapse')"
LIVEKIT_TAG="$(image_tag 'livekit/livekit-server')"
COLLABORA_IMAGE_TAG="$(image_tag 'collabora/code')"
GARAGE_TAG="$(image_tag 'dxflrs/garage')"

pin synapse          https://github.com/element-hq/synapse.git       "$SYNAPSE_TAG"
pin livekit          https://github.com/livekit/livekit.git          "$LIVEKIT_TAG"
pin collabora-online https://github.com/CollaboraOnline/online.git   "$(collabora_repo_tag "$COLLABORA_IMAGE_TAG")"
pin garage           https://git.deuxfleurs.fr/Deuxfleurs/garage.git "$GARAGE_TAG"

echo
echo "Engines pinned under $ENGINES_DIR (read-only reference — CLAUDE.md standing rules)."
