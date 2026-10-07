use crate::error::{PdfError, PdfResult};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EmbeddingPermission {
    Installable,
    Restricted,
    PreviewPrint,
    Editable,
}

/// Technical embedding flags, not a replacement for the font's license.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FontEmbeddingRights {
    version: u16,
    raw_fs_type: u16,
    permission: EmbeddingPermission,
    no_subsetting: bool,
    bitmap_only: bool,
}

impl FontEmbeddingRights {
    pub fn version(&self) -> u16 {
        self.version
    }
    pub fn raw_fs_type(&self) -> u16 {
        self.raw_fs_type
    }
    pub fn permission(&self) -> EmbeddingPermission {
        self.permission
    }
    pub fn no_subsetting(&self) -> bool {
        self.no_subsetting
    }
    pub fn bitmap_only(&self) -> bool {
        self.bitmap_only
    }
    /// Reads the shared OS/2 prefix only; does not validate all metrics fields.
    pub fn from_os2_prefix(data: &[u8]) -> PdfResult<Self> {
        let prefix = data
            .get(..10)
            .ok_or_else(|| PdfError::FontError("Truncated OS/2 embedding flags".into()))?;
        let version = u16::from_be_bytes([prefix[0], prefix[1]]);
        if version > 5 {
            return Err(PdfError::UnsupportedFeature("OS/2 table version".into()));
        }
        let raw_fs_type = u16::from_be_bytes([prefix[8], prefix[9]]);
        let flags = if version <= 1 {
            raw_fs_type & 0x000f
        } else {
            raw_fs_type
        };
        let permissions = flags & 0x000e;
        if flags & !0x030e != 0 || (version >= 3 && permissions.count_ones() > 1) {
            return Err(PdfError::FontError("Invalid OS/2 fsType flags".into()));
        }
        // Legacy versions 0–2 permit the least-restrictive indicated permission.
        let permission = if permissions & 8 != 0 {
            EmbeddingPermission::Editable
        } else if permissions & 4 != 0 {
            EmbeddingPermission::PreviewPrint
        } else if permissions & 2 != 0 {
            EmbeddingPermission::Restricted
        } else {
            EmbeddingPermission::Installable
        };
        Ok(Self {
            version,
            raw_fs_type,
            permission,
            no_subsetting: flags & 0x0100 != 0,
            bitmap_only: flags & 0x0200 != 0,
        })
    }

    pub fn validate_outline_embedding(
        &self,
        subset: bool,
        editable_document: bool,
    ) -> PdfResult<()> {
        if self.permission == EmbeddingPermission::Restricted {
            return Err(PdfError::UnsupportedFeature(
                "font fsType prohibits embedding".into(),
            ));
        }
        if self.bitmap_only {
            return Err(PdfError::UnsupportedFeature(
                "font fsType permits bitmap embedding only".into(),
            ));
        }
        if subset && self.no_subsetting {
            return Err(PdfError::UnsupportedFeature(
                "font fsType prohibits subsetting".into(),
            ));
        }
        if editable_document && self.permission == EmbeddingPermission::PreviewPrint {
            return Err(PdfError::UnsupportedFeature(
                "font fsType permits read-only preview/print documents only".into(),
            ));
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn table(version: u16, flags: u16) -> [u8; 10] {
        let mut data = [0; 10];
        data[..2].copy_from_slice(&version.to_be_bytes());
        data[8..].copy_from_slice(&flags.to_be_bytes());
        data
    }

    #[test]
    fn permissions_and_restrictions_are_enforced() {
        let installable = FontEmbeddingRights::from_os2_prefix(&table(4, 0)).unwrap();
        assert!(installable.validate_outline_embedding(true, true).is_ok());
        let restricted = FontEmbeddingRights::from_os2_prefix(&table(4, 2)).unwrap();
        assert!(restricted.validate_outline_embedding(false, false).is_err());
        let preview = FontEmbeddingRights::from_os2_prefix(&table(4, 4)).unwrap();
        assert!(preview.validate_outline_embedding(false, false).is_ok());
        assert!(preview.validate_outline_embedding(false, true).is_err());
        let no_subset = FontEmbeddingRights::from_os2_prefix(&table(4, 0x108)).unwrap();
        assert!(no_subset.no_subsetting());
        assert!(no_subset.validate_outline_embedding(false, true).is_ok());
        assert!(no_subset.validate_outline_embedding(true, true).is_err());
        let bitmap = FontEmbeddingRights::from_os2_prefix(&table(4, 0x208)).unwrap();
        assert!(bitmap.bitmap_only());
        assert!(bitmap.validate_outline_embedding(false, false).is_err());
    }

    #[test]
    fn legacy_permissions_and_undefined_bits_follow_version_rules() {
        for version in 0..=2 {
            let rights = FontEmbeddingRights::from_os2_prefix(&table(version, 0x0e)).unwrap();
            assert_eq!(rights.permission(), EmbeddingPermission::Editable);
        }
        for version in 0..=1 {
            let rights = FontEmbeddingRights::from_os2_prefix(&table(version, 0xfff8)).unwrap();
            assert_eq!(rights.raw_fs_type(), 0xfff8);
            assert!(!rights.no_subsetting());
            assert!(!rights.bitmap_only());
            assert!(rights.validate_outline_embedding(true, true).is_ok());
        }
        for flags in [1, 0x0c, 0x0010, 0x0400] {
            assert!(FontEmbeddingRights::from_os2_prefix(&table(4, flags)).is_err());
        }
        for end in 0..10 {
            assert!(FontEmbeddingRights::from_os2_prefix(&table(4, 0)[..end]).is_err());
        }
        assert!(FontEmbeddingRights::from_os2_prefix(&table(6, 0)).is_err());
    }
}
