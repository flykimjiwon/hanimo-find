# Fixture Catalog

All fixtures are synthetic. Text files committed directly use LF. Byte sequences
and filesystem objects Git cannot preserve portably are described by manifests:

- `encoding/manifest.json` supplies exact base64 source bytes and canonical
  content for LF, CRLF, and invalid UTF-8 cases.
- `security/manifest.json` describes ignored, hidden, secret, stale, traversal,
  and symlink cases without creating an unsafe link.

A conforming harness decodes manifest `source_base64` into a temporary isolated
root, creates only the declared safe test objects, and compares the expected
representation and exit. It must never materialize a traversal destination or
follow the manifest-described symlink.
