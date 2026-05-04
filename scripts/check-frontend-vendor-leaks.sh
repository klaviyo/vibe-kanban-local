#!/usr/bin/env bash
# Bundle-audit: fails if vendor outbound/analytics identifiers leak into the
# local-web frontend (the VKL local SQLite cutover frontend) or its shared
# library `web-core`.
#
# This is the automated guard for Round 1 implementation-review CRITICAL #8
# on PR #9 ("[SMS2-794] VKL — Strip frontend outbound dependencies and
# analytics imports"). The Parent 4 Definition of Done states the local
# frontend bundle must contain no Electric/wa-sqlite payload and no vendor
# analytics. The DoD is checked against the source tree of `local-web` and
# its only first-party transitive `web-core`. `remote-web` is the legacy
# cloud frontend and is intentionally excluded.
#
# We audit source rather than build output because the project's lint/check
# pipeline does not run a production `vite build`; running a build solely
# for this audit would be far heavier than necessary. A source-level grep is
# sufficient because tree-shaking cannot pull in modules whose import
# specifiers do not appear in source.
#
# If a future maintainer adds a build-output audit, the same identifier list
# here should be reused.
set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"

# Packages that ship in the local frontend bundle.
SCAN_DIRS=(
  "packages/local-web/src"
  "packages/web-core/src"
)

# Vendor outbound/analytics identifiers that must not appear in the local
# frontend bundle. Each entry is a literal substring matched
# case-sensitively across the source tree. Keep this list in sync with the
# deps removed from packages/web-core/package.json and
# packages/local-web/package.json by PR #9.
FORBIDDEN=(
  "posthog-js"
  "posthog/react"
  "@sentry/react"
  "@sentry/vite-plugin"
  "electric-sql"
  "@electric-sql/"
  "@tanstack/electric-db-collection"
  "@tanstack/react-db"
  "wa-sqlite"
)

# Files that legitimately reference these identifiers (e.g., this audit
# script itself, or a documented compatibility shim). Add full repo-relative
# paths only with justification in the same commit.
ALLOWLIST=(
  "scripts/check-frontend-vendor-leaks.sh"
)

is_allowlisted() {
  local path="$1"
  for entry in "${ALLOWLIST[@]}"; do
    if [ "$path" = "$entry" ]; then
      return 0
    fi
  done
  return 1
}

echo "▶️  Auditing local frontend source for vendor outbound/analytics leaks..."

found_any=0
for ident in "${FORBIDDEN[@]}"; do
  hits="$(
    grep -RIn --binary-files=without-match -F -- "$ident" \
      "${SCAN_DIRS[@]/#/$REPO_ROOT/}" 2>/dev/null || true
  )"
  if [ -z "$hits" ]; then
    continue
  fi

  # Filter allowlisted files.
  filtered=""
  while IFS= read -r line; do
    [ -z "$line" ] && continue
    file_path="${line%%:*}"
    rel_path="${file_path#"$REPO_ROOT"/}"
    if is_allowlisted "$rel_path"; then
      continue
    fi
    filtered+="$line"$'\n'
  done <<< "$hits"

  if [ -n "${filtered%$'\n'}" ]; then
    echo "❌ Found leak of '$ident':"
    printf '%s' "$filtered"
    found_any=1
  fi
done

# Also audit the package.json manifests so dependency declarations alone are
# caught even when no source imports them yet (a regression vector).
echo "▶️  Auditing package.json manifests for forbidden dependency declarations..."
for pkg in packages/local-web/package.json packages/web-core/package.json; do
  full="$REPO_ROOT/$pkg"
  [ -f "$full" ] || continue
  for ident in "${FORBIDDEN[@]}"; do
    if grep -q -F -- "\"$ident\"" "$full"; then
      echo "❌ $pkg declares forbidden dependency entry: $ident"
      found_any=1
    fi
    # Also catch scoped/prefix declarations like "@electric-sql/foo"
    case "$ident" in
      */)
        if grep -qE "\"${ident//\//\\/}[^\"]+\"" "$full"; then
          echo "❌ $pkg declares forbidden dependency entry under prefix: $ident"
          found_any=1
        fi
        ;;
    esac
  done
done

if [ "$found_any" -ne 0 ]; then
  echo ""
  echo "Round 1 implementation-review CRITICAL #8: vendor outbound/analytics"
  echo "identifiers must not appear in local-web or web-core. Strip the"
  echo "import or remove the dependency. If a reference is intentional, add"
  echo "the file path to ALLOWLIST in scripts/check-frontend-vendor-leaks.sh"
  echo "with a justification comment."
  exit 1
fi

echo "✅ No vendor outbound/analytics identifiers found in local frontend source."
