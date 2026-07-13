# Encoding Fixtures

`lf.txt` is a normal Git-tracked LF source. `manifest.json` is normative for
exact bytes. It represents CRLF and invalid UTF-8 as base64 so checkout settings,
editors, and JSON Unicode rules cannot alter them. Invalid bytes must round-trip
as base64 and must never become U+FFFD replacement characters.
