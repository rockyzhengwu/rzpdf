# Associated files

The v0.14 API reads explicit document/page associations, creates or clones embedded
Filespec objects, and links or unlinks them from a selected owner. It does not
infer relationships, open external files, run attachments, or certify PDF/A
compliance. Scope and final verification are recorded separately in the release
boundary and acceptance record.

## Read an existing association

```rust,no_run
use pdf::{budget::OperationBudget, document::Document};

# fn main() -> Result<(), Box<dyn std::error::Error>> {
let doc = Document::open("input.pdf")?;
let budget = OperationBudget::new(128 * 1024 * 1024);
let owner = doc.associated_file_owner(Some(0), &budget)?; // None: Catalog
let snapshot = doc.associated_files(&owner, &budget)?;
for file in snapshot.files() {
    println!("{}: {}", file.name(), file.relationship());
}
if !snapshot.files().is_empty() && snapshot.files()[0].is_embedded() {
    let payload = doc.associated_file_data(&snapshot, 0, &budget)?;
    // Inspect bytes explicitly. No filename is used as an output path.
    println!("{} bytes", payload.as_slice().len());
}
# Ok(()) }
```

The snapshot and returned `BudgetedBytes` retain reservations until dropped.
Listing does not decode payloads, although loading stream metadata can parse and
copy encoded source bytes. Foreign or stale targets/snapshots refuse. Reacquire
both after any document revision, including a failed transaction that invalidates
stamps. An `ObjectHandle` is an address, not an immutable snapshot.

Names prefer UF over F. The selected AF profile uses EF/UF when present, otherwise
EF/F, requires a matching filename field, and refuses different F/UF payloads or
multi-platform EF entries. These restrictions do not change the older attachment
and annotation extraction APIs. Unknown relationship names remain strings and
are never rewritten as Unspecified. External Filespecs can be listed but cannot
be extracted or cloned by this API.

## Create or clone an embedded file

```rust,no_run
use pdf::{
    budget::OperationBudget,
    document::{Document, associated_files::{AssociatedFileDefinition,
        AssociatedFileRelationship}, save_options::PdfVersion},
};

# fn main() -> Result<(), Box<dyn std::error::Error>> {
let doc = Document::new_with_version(PdfVersion::V2_0)?;
let budget = OperationBudget::new(128 * 1024 * 1024);
let definition = AssociatedFileDefinition::new(
    "source.csv", "text/csv", AssociatedFileRelationship::Data,
);
let filespec = doc.create_associated_file(&definition, b"name,value\nA,1\n", &budget)?;
let owner = doc.associated_file_owner(None, &budget)?;
assert!(doc.bind_associated_file(&owner, &filespec, &budget)?);

// Or explicitly clone a previously embedded attachment:
doc.add_attachment("old.txt", b"source".to_vec())?;
let old = doc.attachment_filespec("old.txt")?.unwrap();
let copy = doc.clone_associated_file(&old, &AssociatedFileDefinition::new(
    "source.txt", "text/plain", AssociatedFileRelationship::Source,
), &budget)?;
# let _ = (filespec, copy);
# Ok(()) }
```

Creation alone returns an **unlinked** Filespec object. The explicit bind above
adds a Catalog AF link, not an EmbeddedFiles name. To select a page, obtain a
fresh owner using `Some(page_index)`. A second bind of the same identity returns
false without changing the revision; distinct Filespecs with equal filenames
remain distinct. Reacquire the owner after the first bind or any other edit.

`unbind_associated_file(&owner, &filespec, &budget)` removes one owner's link and
returns false if absent. The last removal drops the AF key. It does not delete
the Filespec, payload, EmbeddedFiles name, attachment annotation, another owner's
link or incremental history. Arrays are copy-on-write: old aliases stay intact.
Direct Filespecs can be read but have no stable unlink handle; clone explicitly
to obtain a new indirect Filespec rather than matching by display name.

Cloning creates a new Filespec and a new decoded stream, retaining unrelated
private dictionary fields and the logical payload. It does not change the old
name, file attachment annotation, other owner, relationship, or encoded stream.
Unsupported platform/related-file/encrypted-payload variants refuse. If Params
exists, cloning requires ModDate and validates any Size against decoded length;
no filesystem date is invented. The caller supplies the new name, description,
relationship and MIME explicitly.

Writes require effective PDF 2.0, an unsigned source, and content-modification
permission. Explicitly upgrade and reopen older documents first. Recovery gains
no write API, and signed-file controlled workflows gain no AF authorization.
Failure rolls back inserted objects; unlinking or removing an attachment name
is not physical deletion or secure erasure.

The [boundary](releases/v0.14-boundaries.md) specifies MIME syntax, text/count
caps, shared budgets and untracked allocations. [Current phase results](releases/v0.14-acceptance.md)
distinguish final runs from earlier candidates. See also the [plan](releases/v0.14.md) and
[object-preservation contract](object-preservation.md).
