#!/usr/bin/env bash
set -euo pipefail

cd "$(dirname "${BASH_SOURCE[0]}")/.."

required_commands=(cargo cargo-deny cargo-about cargo-cyclonedx gitleaks jq tar)
for command_name in "${required_commands[@]}"; do
  if ! command -v "${command_name}" >/dev/null 2>&1; then
    echo "release gate: required command is missing: ${command_name}" >&2
    exit 1
  fi
done

required_assets=(
  LICENSE README.md SECURITY.md SPEC.md RESEARCH.md ROADMAP.md BENCHMARK.md
  PROVENANCE.md NAME_REVIEW.md PUBLICATION_DECISION.md THIRD_PARTY_LICENSES.md
  schema/evidence-bundle.schema.json
  schema/query-plan.schema.json
  schema/rag-diagnosis.schema.json
  conformance/v0.constants.json
  conformance/evidence-bundle.accepted.json
  conformance/evidence-bundle.rejected.json
  conformance/query-plan.valid.json
  conformance/rag-diagnosis.valid.json
)
for asset in "${required_assets[@]}"; do
  if [[ ! -s "${asset}" ]]; then
    echo "release gate: required release asset is missing or empty: ${asset}" >&2
    exit 1
  fi
done

./scripts/sync-package-assets.sh --check

cargo fmt --all -- --check
cargo check --workspace --all-targets --all-features --locked
cargo clippy --workspace --all-targets --all-features --locked -- -D warnings
cargo test --workspace --all-targets --all-features --locked
cargo deny check advisories bans licenses sources

notice_tmp="$(mktemp)"
package_tmp="$(mktemp -d)"
sbom_tmp="$(mktemp -d)"
cleanup() {
  rm -f "${notice_tmp}"
  rm -rf "${package_tmp}" "${sbom_tmp}"
}
trap cleanup EXIT

cargo about generate about.hbs --output-file "${notice_tmp}"
if ! cmp -s "${notice_tmp}" THIRD_PARTY_LICENSES.md; then
  echo "release gate: THIRD_PARTY_LICENSES.md is stale; regenerate with cargo-about" >&2
  diff -u THIRD_PARTY_LICENSES.md "${notice_tmp}" || true
  exit 1
fi

package_args=(--locked)
if [[ "${RELEASE_ALLOW_DIRTY:-0}" == "1" ]]; then
  package_args+=(--allow-dirty)
fi
package_assets=(
  LICENSE README.md SECURITY.md SPEC.md RESEARCH.md ROADMAP.md BENCHMARK.md
  PROVENANCE.md NAME_REVIEW.md PUBLICATION_DECISION.md
  schema/evidence-bundle.schema.json
  schema/query-plan.schema.json
  schema/rag-diagnosis.schema.json
)
for package in hanimo-core hanimo-find; do
  cargo package --package "${package}" --list "${package_args[@]}" >"${package_tmp}/${package}.list"
  grep -Fxq Cargo.toml "${package_tmp}/${package}.list"
  grep -Fxq Cargo.lock "${package_tmp}/${package}.list"
  for asset in "${package_assets[@]}"; do
    grep -Fxq "${asset}" "${package_tmp}/${package}.list"
  done
  grep -Eq '^src/.+\.rs$' "${package_tmp}/${package}.list"
done
cargo package --package hanimo-core --no-verify "${package_args[@]}"
cargo package \
  --package hanimo-find \
  --no-verify \
  --config 'patch.crates-io.hanimo-core.path="crates/hanimo-core"' \
  "${package_args[@]}"
for package in hanimo-core hanimo-find; do
  version="$(cargo metadata --no-deps --format-version 1 | jq -r --arg package "${package}" '.packages[] | select(.name == $package) | .version')"
  archive="target/package/${package}-${version}.crate"
  tar -tzf "${archive}" >"${package_tmp}/${package}.archive.list"
  for asset in "${package_assets[@]}"; do
    grep -Fxq "${package}-${version}/${asset}" "${package_tmp}/${package}.archive.list"
  done
done

./scripts/generate-sboms.sh "${sbom_tmp}/generated"
for package in hanimo-core hanimo-find; do
  generated="${sbom_tmp}/generated/${package}.cdx.json"
  committed="sbom/${package}.cdx.json"
  jq -e '.bomFormat == "CycloneDX" and .specVersion == "1.5" and (.components | length > 0)' "${generated}" >/dev/null
  if ! cmp -s "${generated}" "${committed}"; then
    echo "release gate: ${committed} is stale; regenerate it with scripts/generate-sboms.sh" >&2
    exit 1
  fi
done

gitleaks dir . --redact --exit-code 1 --no-banner

echo "release gate: all checks passed"
