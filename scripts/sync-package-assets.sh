#!/usr/bin/env bash
set -euo pipefail

cd "$(dirname "${BASH_SOURCE[0]}")/.."

mode="${1:---check}"
if [[ "${mode}" != "--check" && "${mode}" != "--write" ]]; then
  echo "usage: $0 [--check|--write]" >&2
  exit 2
fi

packages=(hanimo-core hanimo-find)
assets=(
  LICENSE
  README.md
  SECURITY.md
  SPEC.md
  RESEARCH.md
  ROADMAP.md
  BENCHMARK.md
  PROVENANCE.md
  NAME_REVIEW.md
  PUBLICATION_DECISION.md
  THREAT_MODEL.md
  FAQ.md
  docs/MCP.md
  docs/CONSUMING_EVIDENCE.md
  schema/evidence-bundle.schema.json
  schema/query-plan.schema.json
  schema/rag-diagnosis.schema.json
)

for package in "${packages[@]}"; do
  package_root="crates/${package}"
  for asset in "${assets[@]}"; do
    source_path="${asset}"
    destination_path="${package_root}/${asset}"
    if [[ "${mode}" == "--write" ]]; then
      mkdir -p "$(dirname "${destination_path}")"
      cp "${source_path}" "${destination_path}"
    elif [[ ! -f "${destination_path}" ]] || ! cmp -s "${source_path}" "${destination_path}"; then
      echo "package asset is missing or stale: ${destination_path}" >&2
      echo "run ./scripts/sync-package-assets.sh --write" >&2
      exit 1
    fi
  done
done

echo "package assets ${mode#--}: synchronized"
