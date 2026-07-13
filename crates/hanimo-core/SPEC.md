# Hanimo Find v0 Protocol Specification

Status: implemented and frozen contract for schema version `0.1.0`. This
document defines the portable, deterministic v0 boundary implemented by the
Rust source under `crates/hanimo-core` and `crates/hanimo-find`. Indexes, AST
extraction, embeddings, LLM generation, and network transports remain outside
v0.

## 1. Authority and compatibility

JSON is the authoritative interchange format. Markdown is only a pure render of
the same JSON and must not add facts. Every JSON document has
`"schema_version":"0.1.0"`; an unsupported version is an invalid bundle.

## 2. Raw bytes, lines, offsets, and paths

Sources are byte strings. Implementations must not perform lossy decoding or
Unicode normalization.

- Lines are 1-based and inclusive. Byte ranges are 0-based and half-open.
- A block byte range begins at its first content byte and ends one byte after its
  last content byte; the selected final line terminator is excluded.
- LF (`0a`) terminates a line. CRLF (`0d0a`) is one terminator. Neither byte is
  part of line content. A bare CR is content.
- An empty file has zero lines. Consecutive terminators create empty interior
  lines. A final terminator does not create an extra terminal line.
- Canonical block content is the selected line contents, joined by exactly one
  LF, with no final LF. Thus LF and CRLF sources yield identical canonical
  content while raw source hashes remain distinct.
- JSON byte-bearing values use either `{"encoding":"utf8","text":"..."}` or
  `{"encoding":"base64","bytes":"..."}`. Base64 is RFC 4648 canonical base64.
- Evidence paths are raw root-relative path bytes with `/` separators. Absolute
  paths, empty components, `.`, `..`, platform prefixes, NUL, and paths escaping
  the root are forbidden. Symlinks are never followed.

`source_sha256` is the lowercase 64-hex SHA-256 digest of the exact complete raw
file, separate from the block identity.

`bundle_sha256` is a deterministic self-consistency digest, not a signature or
proof of authorship. Its preimage begins with the exact domain
`hanimo:evidence-bundle:v1\0`, followed by `frame(x) = u64be(byte_length(x)) || x`
for the schema version, query, ordered block count and every ordered block field
(raw path, line and byte bounds, raw canonical content, matched terms, score and
components, reasons, block ID, source SHA-256), ordered skips, and critic fields.
List counts and integers are canonical ASCII decimal frames. The digest field
itself, display-only root, and runtime budget are excluded.

The display-only root never grants filesystem authority. The verifier receives a
caller-selected root capability, defaulting to the current directory in the CLI,
and requires its UTF-8 command-line spelling to equal the recorded display value.
Mismatch, parent traversal, or a final symlink fails before source reads (exit 5).

This attestation detects accidental change or a partial bundle rewrite. An
attacker able to rewrite the source, all covered bundle fields, and
`bundle_sha256` can produce a new self-consistent bundle. Preventing that needs a
separately trusted digest, append-only Claim ledger, or signature, which is not
part of v0. `Verified` means the attested evidence payload is unchanged and is
consistent with the current source; it does not establish who authored it.

## 3. Block identity

The block digest preimage is this exact byte sequence:

```
UTF8("imnotrag:block:v1") || 00 ||
frame(path_bytes) ||
frame(ascii_decimal(line_start)) ||
frame(ascii_decimal(line_end)) ||
frame(canonical_content_bytes)
```

`frame(x)` is `u64be(byte_length(x)) || x`. Line numbers use canonical positive
ASCII decimal with no leading zero. `block_id` is `sha256:` followed by the
lowercase 64-hex SHA-256 digest of the complete preimage. The domain string is
exactly `imnotrag:block:v1\0`, including its terminating NUL.

## 4. Query plan and budgets

A query plan separates quoted phrases, identifiers, and ordinary terms. Default
budgets are exact integers:

| Field | Default |
| --- | ---: |
| `max_blocks` | 8 |
| `context_lines` | 3 |
| `max_file_bytes` | 1048576 |
| `max_total_bytes` | 16777216 |
| `max_matches` | 1000 |

`max_matches` counts every exact byte occurrence of each configured non-empty
literal entry. Occurrences are identified by their byte start offset, so
overlapping occurrences count separately (`aa` occurs twice in `aaa`). Repeated
identical entries in the query plan are counted independently.

Search also enforces fixed, non-serialized work ceilings before scanning:

| Ceiling | Maximum |
| --- | ---: |
| UTF-8 query bytes | 4096 |
| exact literal entries | 64 |
| aggregate typed-literal bytes | 4096 |
| eligible candidate files | 64 |
| metadata discovery entries | 256 |
| metadata discovery depth | 64 |

The raw query and typed literal set are both checked so direct core callers and
CLI/MCP callers receive the same bound. A query at either maximum is admitted;
one byte or literal above it is a typed usage error (exit 2) before root access.
Literal entries are rejected, never truncated.

Hidden and ignore-matched paths are walker policy exclusions and do not enter
candidate discovery, so they are not enumerated in `skipped`. The same applies
to symlink and non-regular walker entries. A discovered regular path excluded by
secret-name or per-file-size policy is recorded with `secret` or `oversized`.
Byte or match budget exhaustion returns the deterministic prefix already
scanned, records the omitted discovered suffix with `budget`, and forces critic
rejection. Before source reads, the actual ignore-aware walker is the only
metadata stream. It is unsorted, counts every non-root entry it yields, and
retains at most the 256-entry envelope through depth 64. Hidden and ignored
entries excluded by the walker never enter that stream. Entry max+1 or depth
max+1 discards every candidate and secret skip derived from the incomplete
enumeration, then returns only a root-level `budget` gap whose encoded path is
the empty byte string; no source content is read. Only after a within-envelope
walk completes are eligible paths globally sorted by canonical root-relative
raw bytes and the 64-file prefix selected. The first eligible path outside that
candidate prefix is its `budget` boundary.

Search and diagnosis acquire supplied roots component by component. Every
relative component beneath the trusted current directory and every absolute
component is opened no-follow; a final or intermediate symlink fails before its
target becomes scan authority.

Markdown heading context and the `heading` score recognize ATX levels H1 through
H6: one through six leading `#` bytes followed by an ASCII space or end of line.

## 5. Deterministic scoring and order

Each block score is the integer sum of these components:

| Component | Allowed value |
| --- | ---: |
| `exact_phrase` | 0 or 300 |
| `identifier` | 0 or 250 |
| `all_terms` | 0 or 150 |
| `heading` | 0 or 75 |
| `path` | 0 or 50 |
| `proximity` | integer 0 through 40 |

Results sort by score descending, then raw root-relative path bytes ascending,
line ascending, byte offset ascending, and block ID ASCII ascending. No locale,
filesystem order, Unicode collation, or unstable sort may affect the result.

## 6. Critic rule

The critic accepts only when all conditions hold:

1. at least one evidence block is selected; and
2. every quoted phrase and every identifier in the query plan is covered by an
   exact byte occurrence in at least one selected block; and
3. no `budget` gap reports a truncated scan.

Coverage is vacuously true for an empty quoted-phrase or identifier list.
Ordinary terms do not add a critic coverage requirement. A coverage rejection
records uncovered items. A budget rejection may have no uncovered literal; its
cause is the typed `skipped` entry. Both return exit 1.
Verification recomputes the same observable invariant from `blocks`, `skipped`,
`critic.uncovered`, and `critic.verdict`. A self-consistent artifact whose critic
contradicts those fields is forged evidence (exit 4), not verified evidence.
When the recorded rejected verdict is internally consistent, verification may
emit a `verified` live-integrity report, but the public command still exits 1.

## 7. Exit status contract

| Exit | Meaning |
| ---: | --- |
| 0 | accepted and verified |
| 1 | no evidence or critic rejected |
| 2 | usage error |
| 3 | invalid bundle or unsupported schema |
| 4 | stale or forged evidence |
| 5 | scan, security, or I/O failure |

## 8. Security boundary

The scanner stays beneath the declared root. The walker policy-excludes hidden,
ignore-matched, non-regular, and symlink entries before candidate discovery.
The supplied root itself must be a real directory, not a symlink; rejection
occurs before ambient root capability acquisition.
Discovered secret and oversized regular files are reported as typed gaps without
reading or disclosing their contents. Budget truncation is reported and rejected
as specified above. Verification must reopen and hash source bytes. A path
traversal attempt is a security failure (exit 5); a changed digest, mismatched
block ID, or fabricated content is stale/forged evidence (exit 4). Diagnostics
and skipped reasons must not leak secret contents.

Only `NotFound` while reopening a recorded source is stale evidence (exit 4).
Permission denial, symlink refusal, non-regular or oversized file policy, and all
other source open/read errors are typed security/I/O failures (exit 5) and emit no
partial verification report.

Verification reads at most 16,777,216 serialized bundle bytes before JSON
deserialization. The runtime rejects more than 64 blocks, 65,536 skipped entries,
or 4,096 items in any nested evidence/critic array. Artifact budgets may not
exceed 64 blocks, 4,096 context lines, 16,777,216 bytes per file, 67,108,864
total search bytes, or 1,000,000 matches. Live re-attestation independently
admits at most 134,217,728 aggregate source bytes across both mutation-detection
passes. Bundle-envelope and artifact-limit violations are invalid bundle input
(exit 3); an unsafe root-relative locator or exhausted live-read allowance is a
security/I/O failure (exit 5).

Diagnosis sorts eligible root-relative source paths and reopens every component
beneath one root capability without following symlinks. One diagnosis admits at
most 4,096 regular candidates, 1,048,576 bytes from one file, and 16,777,216
source bytes in total. A candidate, per-file, or total-byte limit is a typed
diagnosis failure (exit 5); no partial `RagDiagnosis` is emitted.

Malformed or over-envelope JSON is invalid bundle input (exit 3). Invalid UTF-8
source bytes are valid source data and must be represented with base64, never
replacement text.

## 9. MCP transport

MCP uses `rmcp` over stdio JSON-RPC. Standard output contains protocol frames
only. Logs, diagnostics, and human-readable errors go to standard error. The v0
contract does not define a network server.

The server captures its canonical current directory once at startup as the MCP
search authority. `search_evidence.path` is optional; when present it must be a
non-empty sequence of normal relative directory components beneath that base.
The MCP boundary validates that syntax and returns the lexical base-relative
join without resolving, canonicalizing, or reopening the requested subpath.
Core search owns capability acquisition and opens every directory component
without following symlinks. Absolute paths, `.`, `..`, platform prefixes/root
components, missing or non-directory targets, and final or intermediate
symlinks are errors. Omitting `path` selects the startup base. A request never
creates a caller-selected ambient root capability.

## 10. Conformance and implementation boundary

Files in `conformance/` are normative examples. Files in `fixtures/` are
synthetic inputs, including manifest-described CRLF, invalid-byte, ignored,
hidden, secret, traversal, stale, and symlink cases. Manifest descriptions are
used where Git cannot portably preserve the filesystem object or byte sequence.

The Rust crates, public schemas, conformance vectors, and tests implement and
check this contract. This specification remains the compatibility authority: an
implementation change may not silently redefine schema `0.1.0`. Generated build
output, dependency caches, and unrelated worktree artifacts are not normative
parts of the contract.
