//! Structural trailer fields are not live business roots. Keep this shared by
//! serialization and page-deletion checks so private trailer roots cannot fall
//! through a Catalog-only ownership scan.
pub(crate) fn retained_trailer_key(key: &str) -> bool {
    !matches!(
        key,
        "Prev" | "XRefStm" | "Type" | "W" | "Index" | "Filter" | "DecodeParms" | "Length"
    )
}
