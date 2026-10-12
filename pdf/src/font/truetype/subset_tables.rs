//! Explicit table policy for a static, original-GID, direct-CID PDF subset.
//! This is not a shaping-font policy. Callers must report omitted tables and
//! must rebuild every `Rebuild` table before publishing the font.
use crate::error::{PdfError, PdfResult};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum TableAction {
    Preserve,
    Rebuild,
    Omit,
}

/// Opting in only permits omission of known layout/auxiliary tables. Unknown
/// tables remain errors: their dependencies are not inferred from their tag.
pub(crate) fn table_action(tag: &str, omit_layout: bool) -> PdfResult<TableAction> {
    use TableAction::*;
    match tag {
        "glyf" | "loca" | "cmap" | "head" | "name" => Ok(Rebuild),
        // Original GIDs and numGlyphs are retained. Hint programs and their
        // original conservative resource bounds remain paired with outlines.
        "hhea" | "hmtx" | "maxp" | "OS/2" | "post" | "cvt " | "fpgm"
        | "prep" | "gasp" | "vhea" | "vmtx" | "STAT" => Ok(Preserve),
        // A changed font cannot retain the original content signature.
        "DSIG" => Ok(Omit),
        // These can refer to glyphs not in the direct-CID selection. Retaining
        // GIDs alone does not make their substitutions/positioning valid.
        "GDEF" | "GPOS" | "GSUB" | "kern" | "BASE" | "JSTF"
        // Optional device caches / printer metadata need rebuilding if empty
        // glyph slots change. The direct-CID profile explicitly reports their
        // omission; it does not claim the stale caches are safe to copy.
        | "LTSH" | "hdmx" | "VDMX" | "PCLT" => {
            if omit_layout { Ok(Omit) } else {
                Err(PdfError::FontError(format!("Subset layout/auxiliary omission requires explicit policy: {tag}")))
            }
        }
        // Variable, alternate-outline, color and bitmap tables require their
        // own dependency/rebuild support, not the static glyf path.
        "fvar" | "gvar" | "avar" | "HVAR" | "VVAR" | "MVAR"
        | "CFF " | "CFF2" | "COLR" | "CPAL" | "SVG " | "CBDT" | "CBLC"
        | "EBDT" | "EBLC" | "EBSC" | "sbix" => {
            Err(PdfError::FontError(format!("Unsupported subset font table: {tag}")))
        }
        _ => Err(PdfError::FontError(format!("Unknown subset table policy: {tag}"))),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn original_gid_policy_does_not_blindly_preserve_dependencies() {
        for tag in ["glyf", "loca", "cmap", "head", "name"] {
            assert_eq!(table_action(tag, false).unwrap(), TableAction::Rebuild);
        }
        for tag in [
            "hhea", "hmtx", "maxp", "OS/2", "post", "cvt ", "fpgm", "prep", "gasp", "vhea", "vmtx",
            "STAT",
        ] {
            assert_eq!(table_action(tag, false).unwrap(), TableAction::Preserve);
        }
        assert_eq!(table_action("DSIG", false).unwrap(), TableAction::Omit);
        for tag in [
            "GDEF", "GPOS", "GSUB", "kern", "BASE", "JSTF", "LTSH", "hdmx", "VDMX", "PCLT",
        ] {
            assert!(table_action(tag, false).is_err());
            assert_eq!(table_action(tag, true).unwrap(), TableAction::Omit);
        }
        for tag in ["ZZZZ", "fvar", "gvar", "CFF ", "COLR", "sbix"] {
            assert!(table_action(tag, false).is_err());
            assert!(table_action(tag, true).is_err());
        }
    }
}
