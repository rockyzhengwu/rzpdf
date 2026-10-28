# PDF standard coverage and evidence

This is a capability index for the declared v0.1–v0.14 profiles, not a PDF conformance score. Read, write, render and edit are separate capabilities. A saved opaque object is not a supported semantic editor. Rows below link the applicable standard area to public entry points, executable positive/negative assertions and the detailed version boundary. Version numbers identify the delivery profile, not a claim to implement every feature of that PDF specification version.

## File structure and common objects

| Standard area | Profile and capability | Public entry / assertions | Detailed scope and evidence |
|---|---|---|---|
| 7.2–7.3 lexical objects | v0.1 read/write; Strict rejects malformed/duplicate objects, Compatible has bounded diagnostics | `Document::from_bytes_with_options`, `SyntaxParser`; parser unit tests, [stream_structure](../pdf/tests/stream_structure.rs), [recovery](../pdf/tests/v012_recovery.rs) | [v0.1 support](releases/v0.1-support.md), [diagnostics](file-recovery.md) |
| 7.5.2–7.5.6 headers/xref/history | v0.1 read; v0.5 incremental; v0.12 classic/stream output, source preservation | `save_to_bytes_with_layout`, `save_incremental_to_bytes_with_layout`; [xref](../pdf/tests/v012_xref_output.rs), [incremental](../pdf/tests/v012_incremental_stream.rs), [version refusals](../pdf/tests/feature_versions.rs) | [v0.12 boundaries](releases/v0.12-boundaries.md), [independent gate](check-v012-xref-output.py) |
| 7.5.7–7.5.8 ObjStm/XRef | v0.12 full output and readback; member IDs preserved; prohibited container inputs refuse | `SaveLayout::XrefStreamWithObjectStreams`; [group/member tests](../pdf/tests/v012_object_streams.rs) | [ObjStm independent gate](check-v012-object-streams.py); no incremental packing, hybrid/linearized generation |
| 7.5 damaged indexes | v0.12 explicit read-only recovery; not ordinary Strict/Compatible | `RecoveredDocument::from_bytes`; [paired recovery/limits/refusals](../pdf/tests/v012_recovery.rs), compile-fail API tests | [profile](file-recovery.md), [external gate](check-v012-recovery.py) |
| 7.6 encryption | v0.5/v0.11/v0.12 selected Standard R6 read/write and unsigned incremental | `Document::from_bytes_with_password`, `SaveSecurity`; [security](../pdf/tests/v05_security.rs), [layout protection](../pdf/tests/v012_layout_security.rs), [encrypted incremental](../pdf/tests/v011_encrypted_incremental.rs) | [security matrix](releases/v0.11-security-matrix.md), [external R6 gate](check-v012-layout-security.py); old handlers/public-key and encrypted signatures excluded |
| 7.7/7.9 page tree and resource graphs | v0.1/v0.4 read/edit; v0.9/v0.10 relationship-aware migration | `get_page`, `copy_page`, `import_pages_from`, `delete_page`; [page relationships](../pdf/tests/v09_page_relationships.rs), [structure transfer](../pdf/tests/v010_structure_transfer.rs) | [page guide](page-relationships.md); unknown ownership fails atomically |
| 7.9 name/number trees | v0.9 read/write/edit, ordering/Limits/cycle bounds | named destinations, page labels and attachments; [navigation](../pdf/tests/v09_navigation.rs), [relationships](../pdf/tests/v09_page_relationships.rs) | [navigation](navigation.md), [v0.9 support](releases/v0.9-support.md) |
| 7.11 and 14.13 files/associated files | v0.14 selected PDF 2.0 Catalog/page AF read/create/link/unlink, page transfers and protected saves; legacy attachment API retained | `associated_files`, `associated_file_data`, `create_associated_file`, `clone_associated_file`, `bind_associated_file`, `unbind_associated_file`; [tests](../pdf/tests/v014_associated_files.rs) | [guide](associated-files.md), [boundary](releases/v0.14-boundaries.md), [independent gate](check-v014-associated-files.py) |
| 14.3 Info/XMP | v0.1 raw metadata and selected Info editing; preserve unrelated data | `metadata_packet`, `set_metadata_packet`, `info_dict`; [metadata](../pdf/tests/metadata.rs), [private roots](../pdf/tests/v012_preservation.rs) | No full XML schema/synchronization; [operation matrix](object-preservation.md) |

## Graphics and text

| Standard area | Profile and capability | Public entry / positive and negative assertions | Detailed scope and evidence |
|---|---|---|---|
| 7.4 stream filters/images | v0.1/v0.6 bounded decoding and selected embedding/extraction | filter API and image resource methods; [codecs](../pdf/tests/v06_codecs.rs), [PNG](../pdf/tests/png_embedding.rs), [JPEG](../pdf/tests/jpeg_embedding.rs), [image extraction](../pdf/tests/image_extraction.rs) | [v0.6 matrix](releases/v0.6-matrix.md), [acceptance](releases/v0.6-acceptance.md); limits and unsupported sample precision remain explicit |
| 8.4–8.5 graphics state/path | v0.1/v0.2 interpretation/render and source-addressed path edits | `display_list`, path edit APIs; [path edit](../pdf/tests/path_edit.rs), [nested paths](../pdf/tests/nested_path_edit.rs), [extgstate](../pdf/tests/extgstate.rs) | [v0.2 support](releases/v0.2-support.md), [v0.4 support](releases/v0.4-support.md) |
| 8.6 color spaces | v0.8 RGB display, actual ICC conversion and Alternate tint simulation | color interpretation/render contexts; [color](../pdf/tests/v08_color.rs), [workflow](../pdf/tests/color_workflow.rs), ICC unit tests | [color guide](color-management.md), [v0.8 support](releases/v0.8-support.md); no print separation/automatic OutputIntent |
| 8.7 patterns | v0.8 selected tiling and shading patterns; explicit matrices/resources | DisplayList/replay; [patterns](../pdf/tests/v08_pattern.rs), render pattern unit tests | [patterns](patterns.md), [v0.8 acceptance](releases/v0.8-acceptance.md); not arbitrary combinations/performance |
| 8.7.4 shading types 1–7 | v0.8 selected numeric/geometric profiles; work exhaustion refuses | shading functions and replay; [tensor](../pdf/tests/v08_tensor.rs), [shading workflow](../pdf/tests/shading_workflow.rs), tensor unit tests | [v0.8 support](releases/v0.8-support.md); difficult patches may ResourceLimit |
| 8.9/8.10 images and Form XObjects | v0.1/v0.4/v0.6 interpret/render/register/edit selected invocations | image and Form source APIs; [replacement](../pdf/tests/image_replacement.rs), [Form replacement](../pdf/tests/form_replacement.rs), [image geometry](../pdf/tests/image_hit.rs) | [v0.4 support](releases/v0.4-support.md), no inferred layout |
| 9.4 text operators/geometry | v0.1/v0.2/v0.3 selected interpretation, extraction and edits | `extract_characters_paint_order`, text event edit APIs; [geometry](../pdf/tests/text_geometry.rs), [event range](../pdf/tests/text_event_range.rs), [adjustments](../pdf/tests/adjusted_text.rs) | [v0.2 support](releases/v0.2-support.md), [v0.3 support](releases/v0.3-support.md); paint order is not semantic reading order |
| 9.5–9.10 fonts/encodings | v0.3/v0.7 selected simple/CID/Type3 reading, TT embedding/subset, explicit glyph runs | font/resource APIs; [mapping](../pdf/tests/v07_font_mapping.rs), [glyph runs](../pdf/tests/v07_glyph_run.rs), [Type3](../pdf/tests/v07_type3_model.rs), subset unit tests | [font matrix](releases/v0.7-font-matrix.md); shaping/fallback, unsupported outlines and full Type3 font authoring excluded |
| 9.3.6 text clipping | v0.8 TrueType actual outlines, Tr4–7 | DisplayList/replay and text clip unit tests | [v0.8 support](releases/v0.8-support.md); Type1/CFF/Type3 outline clipping refuses, no bounding-box substitution |
| 11 transparency | Selected RGB groups, masks and 16 blend modes; isolated matrix/TRC ICC RGB groups added in compatibility fixes | interpreter/replay; [transparency](../pdf/tests/v08_transparency.rs), [ICC group tests](../render/src/icc_group_tests.rs), [compatibility checks](testing.md#icc-透明组和-wasm-转换表回归) | [guide](transparency.md), historical [v0.8 acceptance](releases/v0.8-acceptance.md); ICC Gray/CMYK and LUT-only groups, ICC soft-mask spaces, overprint, AIS=true/TK=false excluded |

## Interactive and document interchange

| Standard area | Profile and capability | Public entry / positive and negative assertions | Detailed scope and evidence |
|---|---|---|---|
| 12.3 navigation | v0.9 selected destinations/bookmarks/labels | navigation CRUD; [navigation](../pdf/tests/v09_navigation.rs), [relationships](../pdf/tests/v09_page_relationships.rs) | [navigation guide](navigation.md); remote/chained actions not general editing |
| 12.5 annotations/AP | v0.4/v0.9 selected subtype CRUD, relationships and explicit preview | annotation APIs and `display_with_annotations`; [annotations](../pdf/tests/v09_annotations.rs), render annotation tests | [guide](annotations.md), [v0.9 support](releases/v0.9-support.md); ASCII FreeText subset, unknown ownership refuses |
| 12.7 AcroForm | v0.4/v0.9 hierarchical fields, multiple Widgets and selected values/AP | form APIs; [forms](../pdf/tests/v09_forms.rs), [field types](../pdf/tests/v09_form_types.rs) | [forms](forms.md), [v0.9 acceptance](releases/v0.9-acceptance.md); no XFA/JavaScript or automatic layout |
| 12.8 signatures/CMS | v0.5/v0.11 detached signing/verification and bounded history | signature preparation/completion/verification APIs; [signatures](../pdf/tests/v05_signatures.rs), [history](../pdf/tests/v011_signature_history.rs), [multisign](../pdf/tests/v011_multisign.rs) | [security matrix](releases/v0.11-security-matrix.md), [CMS gate](check-v011-signatures.py), [multisign gate](check-v011-multisign.py); no network trust/LTV/PAdES claim |
| 12.8 MDP and controlled updates | v0.11 explicit selected field/signature/Square workflows | immutable controlled plans; [updates](../pdf/tests/v011_controlled_updates.rs), [fields](../pdf/tests/v011_signature_fields.rs) | [guide](signed-workflows.md), [independent gate](check-v011-workflows.py); not arbitrary signed-file editing |
| 14.6 marked content | v0.3/v0.10 explicit MCID/ActualText/Artifact | `extract_marked_text`; [marked text](../pdf/tests/v010_marked_text.rs), [source events](../pdf/tests/v03_marked_content.rs) | [v0.10 support](releases/v0.10-support.md); no inferred language/reading order |
| 14.7 logical structure | v0.10 selected tree/associations/semantics/node edits and page transfers | structure APIs and `extract_text_structure_order`; [model](../pdf/tests/v010_structure.rs), [associations](../pdf/tests/v010_structure_associations.rs), [semantics](../pdf/tests/v010_structure_semantics.rs), [edits](../pdf/tests/v010_structure_edit.rs), [transfer](../pdf/tests/v010_structure_transfer.rs) | [migration guide](structure-transfers.md), [v0.10 support](releases/v0.10-support.md); no PDF/UA or complete role inclusion certification |
| 8.11 optional content | v0.10 explicit OCG/OCMD selection; v0.13 selected authoring, accepted within its frozen profile | snapshot/visibility APIs, `create_layer`, `write_layer_configuration`, `create_layer_membership`, segment/call binding; [read tests](../pdf/tests/v010_optional_content.rs), [edit tests](../pdf/tests/v013_optional_content_edit.rs) | [read guide](optional-content.md), [authoring guide](layer-authoring.md), [v0.13 boundary](releases/v0.13-boundaries.md); no OCProperties transfer, flattening or action runtime |

Linearization (Annex F): complete-file reading and recognized-source conversion
to ordinary PDF through all full-save layouts are supported within the
[documented removal profile](advanced-saving.md). See [targeted tests](../pdf/tests/linearized_save.rs)
and the [qpdf/MuPDF gate](check-linearized-save.py). Linearized output, progressive
Range loading and linearized-source incremental saving remain unsupported.

Explicit-font FreeText (12.5): a bounded static TrueType Chinese/Latin appearance
profile now covers native and Web creation, strict save/reopen and updates. See
[API/scope](unicode-freetext.md), [native assertions](../pdf/tests/unicode_freetext.rs),
[real SDK/Worker gate](../web/tests/unicode-freetext-browser.mjs) and
[MuPDF/PDFium gate](check-unicode-freetext.py). Legacy ASCII APIs remain unchanged;
this does not imply arbitrary external FreeText, rich text or shaping support.

## Reproducible evidence and licensing

### Accepted associated file profile

The [v0.14 profile](releases/v0.14.md) is accepted for Catalog/page AF snapshots, explicit extraction, creation/cloning, linking/unlinking, selected page operations and protected saves. See [final evidence](releases/v0.14-acceptance.md), [API guide](associated-files.md) and [standard-source record](releases/v0.14-standard-sources.md). Official public AF notes and errata support this selected profile; a complete ISO PDF was not obtained or certified. StructElem, content-sequence MCAF, arbitrary owners and PDF/A remain excluded.

### Read write render and edit distinctions

Every supported cell below is limited to its linked detailed profile and the assertions above. “Preserve” means retaining an existing representation, not semantic authoring. “Selected” is not support for all valid combinations. A dash means that operation is not an applicable capability, not a passed test.

| Unit | Read | Write | Render | Edit |
|---|---|---|---|---|
| Lexical objects / private dictionaries | Bounded Strict/Compatible | Representation | — | Low-level objects, not unknown schemas |
| xref/history/ObjStm | Tables, streams, hybrid, Prev | Three full / two append layouts | — | New output indexes, not source-container payload editing |
| Damaged-index recovery | Explicit first profile | Forbidden | Detached selected page content | Forbidden |
| R6 | Authenticated selected profile | Preserve/remove/set under identity rules | Supported decrypted page content | Selected unsigned operations / permissions |
| Page tree / resource graphs | Selected validation | Known relationships | Geometry/content | Copy/import/delete/reorder with refusal guards |
| Name/number trees / navigation | Selected models | Balanced supported trees | No action runtime | Selected destinations/bookmarks/labels |
| EmbeddedFiles / AF | Payload API / selected owner snapshots | Payload API / selected AF creation | Selected attachment annotation AP only | Selected Catalog/page bind/unbind/clone and page transfers; not arbitrary owners |
| Info / XMP | Selected Info / raw packet | Selected Info / raw packet | — | Explicit raw replacement, no XML schema synchronization |
| Stream filters / image data | Selected codecs/precision | Selected embedding and encoded-data preserve | Selected image/color/mask profiles | Resource/invocation replacement or removal |
| Graphics state / paths | Selected interpretation | Builder/preserve | Selected raster profile | Source-addressed paths/graphics state |
| Color / ICC / tint | Selected color models | Resource/preserve | RGB display / Alternate simulation | Selected resource construction, no print separation editor |
| Patterns / shading | Selected bounded types | Resource/preserve | Selected types 1–7 / tiling | Resource operations, no inferred mesh editor |
| Form XObjects | Resource/content model | Selected creation/preserve | Selected recursive interpretation | Selected invocation isolation and replacement |
| Text operators | Source events / geometry | Explicit text/glyph runs | Selected font profiles | Selected source-addressed ranges, not automatic layout |
| Fonts / encodings | Selected simple/CID/Type3 | TrueType embedding/subset; other preserve | Selected supported outlines/programs | Fixed-profile runs/mappings, no general font authoring |
| Text clipping | Tr state / selected TT outlines | Representation | TrueType Tr4–7 | Selected content operations, no CFF/Type1 fallback |
| Transparency | Selected state/group/mask model | Representation | Selected RGB groups / masks / blend modes | Selected state/resource operations, not all compositing models |
| Annotation / AP | Selected subtype models | Selected AP synthesis | Explicit Screen/Print preview | Selected CRUD/relationships and refusal guards |
| AcroForm | Selected field/widget hierarchy | Selected values / AP | Selected Widget preview | Selected field types / Unicode profiles |
| CMS / signature history | Bounded verification/history | Detached signing / selected append | Selected explicit AP | General signed-file editing forbidden |
| MDP controlled updates | Selected constraints / revisions | Validated immutable plans only | Selected field/signature/Square AP | Only declared allowed operations |
| Marked content / ActualText | Explicit semantic data | Selected tagging/preserve | Actual content; no automatic Alt substitution | Selected marking and node operations |
| Logical structure | Selected tree/associations | Selected structure operations | No semantic-layout engine | Selected node edits / page transfers with rollback |
| OCG / OCMD | Selected configurations | Selected group/configuration/membership creation; preserve | Explicit visibility selection | v0.13 selected authoring and top-level call binding; no flattening or OCProperties transfer |

The linked integration tests contain positive assertions and refusals/rollback cases; library and render unit tests cover lower-level numeric and malformed-input boundaries. Each detailed release support table remains the authoritative sub-profile: no row above broadens those cells. [v0.12 acceptance](releases/v0.12-acceptance.md) distinguishes current frozen-source runs from earlier version evidence. Re-running the complete workspace checks all retained tests, not every external reader or every PDF feature combination.

External v0.12 recipes are `check-v012-xref-output.py`, `check-v012-object-streams.py`, `check-v012-layout-security.py`, `check-v012-incremental.py`, `check-v012-preservation.py` and `check-v012-recovery.py`. Each creates a new directory, uses locally available producers/readers, and records versions/provenance and hashes. The object-stream gate includes the xref-output gate; do not double-count those nested cases. pikepdf and qpdf share a backend; MuPDF supplies an independent reader/rasterizer. Recovery compares detached Skia pixels against undamaged Strict sources and retains MuPDF references; it never saves a repaired PDF.

The v0.13 recipe [check-v013-layers.py](check-v013-layers.py) uses original pikepdf and MuPDF create inputs, structural projections and three explicit visibility selections. The [additional indirect-dictionary recipe](testing.md) checks preservation of old configuration aliases. Final source results, whole-workspace/joint tests, eleven independent gates and 432 reviewed new-feature output pages are tracked in [v0.13 acceptance](releases/v0.13-acceptance.md); older release counts are not substituted for them.

Recipes and synthetic fixture definitions are original project material under the repository license. Generated test PDF content is synthetic and carries no copied third-party document permission claim. Test fonts and profiles keep their separately documented licenses: see fixture READMEs under `pdf/tests/fixtures/`, the [CMap/font notices](../font-data/NOTICE.md) and each version acceptance record. A tool-generated PDF is not evidence of permission to redistribute an arbitrary input. Existing binary samples without renewed provenance are not promoted to new public redistribution rights.

Standard references: [ISO 32000-2 corrected clauses](https://pdf-issues.pdfa.org/32000-2-2020/) and [clause 7](https://pdf-issues.pdfa.org/32000-2-2020/clause07.html). The coverage index is an implementation mapping, not a substitute for the normative specification or a validator. OCR, untagged table/reading-order inference, automatic layout, multimedia/action execution, PDF/A/UA certification and unselected print/security profiles remain outside this release.
