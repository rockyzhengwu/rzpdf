# Source-backed PDF streams (RZPDF-002)

## Scope and acceptance

Separate object identity/dictionaries from stream payload residency. Keep the
browser default 16 MiB raw cache, 16 MiB decoded stream limit and 64 MiB operation
budget. Do not reopen documents, drop edits, or raise those ceilings to navigate
large image PDFs.

The editor regression fixture is 22,023,982 bytes with SHA-256
`03be863a438eb2b44709947a5a211a9b94d120e231c92bfaae26be521f1f2551`.
It has seven independent raw RGB image pages and seven `Image page` matches.
Acceptance includes repeated mixed navigation, nonblank rendering, search, saving,
source/current views, transactions, shared streams, object streams and encryption.

## Implementation checklist

- [x] Parser records source ranges only after validating Length/endstream (or
  bounded compatible recovery), with opaque source identity.
- [x] Owned immutable byte input uses range views into its existing allocation.
- [x] Arbitrary Read + Seek sources use bounded LRU residency with pinned read
  owners; no new static lifetime requirement on readers.
- [x] Raw cache admission moves from object resolution to payload reads.
- [x] Source encryption captures object-bound decryption settings; payload AES
  processing is deferred until a read. Object-stream members still decrypt once
  through their container.
- [x] Text extraction skips Image XObjects and inline image decoding, while
  continuing Form traversal and optional-content handling.
- [x] Full save removes eager object preload and pins each payload during output.
- [x] Native raw read APIs return owners; non-I/O byte access is fallible.
- [x] Resource diagnostics and browser cache clearing/types documentation.
- [x] Exact fixture in native and browser render/search/save paths.
- [x] Parser, transaction, encryption, object-stream and save regressions.

## API migration

`PdfStream::raw_data()` returns `PdfResult<&[u8]>` and performs no I/O. Newly
constructed streams and immutable input views have resident bytes. Generic source
streams and encrypted streams require `PdfDocument::read_stream_data(&stream)`
or `read_stream_data_with_budget`. Keep the returned `RawStreamData` owner alive
while using its bytes. It dereferences to `[u8]`.

`into_raw_parts()` is also fallible and requires a resident plaintext payload.
For cross-document stream comparisons, compare dictionaries and bytes returned
by each document's read API. `PartialEq` performs no I/O: unresolved ranges compare
source identity and range, while resident payloads compare bytes. Signature
history auditing uses explicit contextual byte comparisons.

Decoded replacement APIs continue to create owned document edits. A cache clear
never removes edits or invalidates active read owners. Clearing caches does not
shrink WebAssembly linear memory.

## Accounting

Immutable input storage is reported once as source input bytes. Range views do
not allocate another payload and do not occupy the owned raw residency budget.
Generic readers cache owned buffers under maxCachedStreamBytes. Outstanding
readers pin their allocation and reservation. A stream larger than that cache is
read with the operation budget instead of being permanently admitted. Decoding
and output still have their independent limits.

A residency admission failure reports `resource_limit`, reason
`raw_stream_residency`, stage `source_read`, with limit and actual bytes. Parser
stream-size failures report `raw_stream_bytes` at `stream_parse`. Neither
indicates that lowering raster DPI necessarily helps.

## Verification

The fixed fixture is reproducible without external assets:

```sh
node scripts/source-stream-fixture.mjs /tmp/rzpdf-source-streams.pdf
cargo test -p pdf --test source_streams
# After the pinned Wasm build and npm --prefix web run build:
node web/tests/source-streams-browser.mjs
```

The native tests cover pinned reads across eviction/clear, operation-scoped
oversized streams, failure/retry after partial source I/O, lazy AES decryption,
immutable input sharing and full save/reopen. A context test exercises source
views, committed edits, tentative edits, rollback and foreign-reader rejection.
Existing object-stream, signature history, encryption and incremental-layout
regressions remain part of the broader acceptance run.

Chromium evidence is in `output/source-streams-20261029/browser-final/report.json`.
Both synchronous SDK and Worker use the default profile: three rounds of the
exact mixed navigation sequence, 30 renders per transport, seven cold/after-round
search hits, unchanged pixels, one retained input allocation, bounded prepared
pages, cache clearing without edit loss, and edited full save/strict reopen.
The raw owned cache peak for this immutable-input fixture is zero; its source
input remains 22,023,982 bytes. This does not mean that decoding or rendering
uses zero memory. The edited saved file is 22,024,367 bytes.

The browser run also exposed an independent SDK bug: binary save output was
subject to the 16 MiB JSON read ceiling. Saves now read against the requested
binary budget (at most the existing 64 MiB shared-buffer ceiling), while JSON
retains its 16 MiB limit. Oversized output reports a structured `buffer_bytes`
error at `sdk_read` instead of an empty internal error. SDK tests cover both
limits and native output handle release on rejection.

Additional optimized-Wasm regressions exercise high-DPI/reduced JPEG resources,
the 9 MiB reader upload/download path, Unicode FreeText including permissions,
and ten ordinary rewrites of linearized input across synchronous/Worker paths.

Native acceptance: 1,713 PDF tests across 96 groups, 115 renderer tests and 25
Wasm host-interface tests passed; one pre-existing test remains ignored. A final
ownership audit also reran the library/source-stream tests and checks that the
internal owned-ciphertext path reserves its copy before allocation. Build-driver
checks, TypeScript compilation, SDK/protocol resource tests, formatting and diff
whitespace checks passed. qpdf independently accepted the fixed input fixture.
These are engine/SDK results; the editor still needs to pin the implementation,
rebuild its Wasm assets and run its own application acceptance.
