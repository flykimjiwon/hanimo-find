#!/usr/bin/env bash
set -euo pipefail

cd "$(dirname "${BASH_SOURCE[0]}")/.."

output_dir="${1:-sbom}"
mkdir -p "${output_dir}"

generated_sboms=()
cleanup() {
  if ((${#generated_sboms[@]})); then
    rm -f "${generated_sboms[@]}"
  fi
}
trap cleanup EXIT

SOURCE_DATE_EPOCH=0 cargo cyclonedx \
  --format json \
  --spec-version 1.5 \
  --all-features \
  --target all \
  --override-filename release-gate.cdx

for package in hanimo-core hanimo-find; do
  generated="crates/${package}/release-gate.cdx.json"
  generated_sboms+=("${generated}")
  destination="${output_dir}/${package}.cdx.json"
  temporary="${destination}.tmp"
  jq -S '
    del(.serialNumber, .metadata.timestamp)
    | walk(
        if type == "string" then
          gsub("path\\+file://[^#]*/crates/"; "path+file://./crates/")
        else
          .
        end
      )
  ' "${generated}" >"${temporary}"
  mv "${temporary}" "${destination}"
done
