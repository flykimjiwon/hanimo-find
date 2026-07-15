# MCP integration guide

`hanimo find mcp` serves a local stdio MCP server over protocol-only stdout.
This document describes client configuration and the exact contract of the
three tools. The normative evidence semantics remain [SPEC.md](../SPEC.md);
nothing here weakens the v0.1 evidence boundary.

## Trust model

The server captures its **canonical startup directory as the trusted base**.
Launch it from the directory it is authorized to search — the working
directory is the authorization decision. Every tool's optional `path` argument
is a nested relative directory beneath that base: only normal relative
components are accepted, the request path is joined lexically without being
resolved or reopened, and core acquisition then opens the target component by
component without following symlinks. Absolute, parent, dot, platform-prefix,
unavailable, non-directory, and symlinked paths are rejected.

The server never writes, requires no network, no daemon, no index, and no
model. One process serves one trusted base; run one server per authorized
root instead of widening a single base.

## Client configuration

### Claude Code

`.mcp.json` in the project whose checkout should be searchable:

```json
{
  "mcpServers": {
    "hanimo-find": {
      "command": "hanimo",
      "args": ["find", "mcp"]
    }
  }
}
```

Claude Code starts the process in the project directory, which becomes the
trusted base. Point `command` at an absolute binary path if `hanimo` is not on
`PATH`.

### Generic stdio clients

Any MCP client that speaks JSON-RPC over stdio can launch `hanimo find mcp`
directly. Use normal JSON-RPC framing; the server keeps stdout protocol-only
and an integrating client must not mix human-readable output into that stream.

## Tools

### `search_evidence`

| Argument | Type | Meaning |
| --- | --- | --- |
| `query` | string, required | Parsed into quoted phrases, uppercase identifiers, and ordinary terms; matching is byte-exact. |
| `path` | string, optional | Relative subdirectory beneath the trusted base. Omitted means the base itself. |

Returns the authoritative EvidenceBundle as structured content — byte-identical
in meaning to `hanimo find search --format json` over the same resolved root.
Query-ceiling violations are parameter errors; scan failures are structured
errors. A bundle whose critic verdict is `rejected` is still returned: the
caller must gate on `critic.verdict`, not on the call succeeding.

### `verify_evidence`

| Argument | Type | Meaning |
| --- | --- | --- |
| `bundle_json` | string, required | The authoritative EvidenceBundle JSON exactly as returned by `search_evidence` or the CLI. Inputs over the 16 MiB verification limit are rejected before parsing. |
| `path` | string, optional | Relative subdirectory beneath the trusted base. The resolved target must equal the bundle's recorded display root. |

Reopens every cited source and returns:

```json
{
  "accepted": true,
  "report": { "status": "verified", "attempts": 1, "blocks": [] }
}
```

`accepted` is `true` only when the live verification status is `verified`
**and** the bundle's critic verdict is `accepted` — the same condition as CLI
exit 0. `stale`, `forged`, and `source_drift` outcomes are returned in-band
with `accepted: false` so an agent can see why its evidence died. Malformed or
schema-unsupported bundles are parameter errors; root mismatch and
security/I/O failures are structured errors.

### `diagnose_repo`

| Argument | Type | Meaning |
| --- | --- | --- |
| `path` | string, optional | Relative subdirectory beneath the trusted base. Omitted means the base itself. |

Runs the versioned static `imnotrag` repository diagnostics without executing
the target and returns the authoritative `RagDiagnosis` JSON: schema version,
source-cited findings, summary, and the deterministic corpus digest.

## The evidence loop

The reason `verify_evidence` exists as a tool: an agent that quotes evidence
should be able to prove the citation still resolves before acting on it.

1. `search_evidence` → hold the returned bundle; cite only its `block_id`,
   `path`, line ranges, and `source_sha256` — never restyled prose.
2. Act on the evidence (answer, edit, review).
3. `verify_evidence` with the original bundle → require `accepted: true`
   before trusting or publishing the citation. A `budget` skip makes a bundle
   critic-rejected even when every byte still matches; partial evidence is
   never promoted to sufficient evidence.

## Limits

- Matching is byte-exact: no case folding, stemming, paraphrase matching, or
  Unicode normalization (NFC/NFD-different text does not match).
- All search, verification, and diagnosis resource envelopes documented in
  [README.md](../README.md) apply unchanged over MCP.
- `bundle_sha256` remains an unkeyed integrity checksum, not a signature; see
  [SECURITY.md](../SECURITY.md) for the trust boundary.
