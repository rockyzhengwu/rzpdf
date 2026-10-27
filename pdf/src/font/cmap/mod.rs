use std::collections::HashMap;

use crate::{error::PdfError, font::CharCode};
use font_data::cmap::get_predefine_cmap_data;
use parser::CmapParser;

use crate::error::PdfResult;

mod parser;

#[derive(Debug, Clone)]
pub(super) struct MappingTable<T, K = u32> {
    hash: HashMap<K, T>,
    sorted: Option<Vec<(K, T)>>,
}

impl<T, K> Default for MappingTable<T, K> {
    fn default() -> Self {
        Self {
            hash: HashMap::new(),
            sorted: None,
        }
    }
}

impl<T, K> From<HashMap<K, T>> for MappingTable<T, K> {
    fn from(hash: HashMap<K, T>) -> Self {
        Self { hash, sorted: None }
    }
}

impl<T, K: Copy + Ord + std::hash::Hash> MappingTable<T, K> {
    pub(crate) fn storage_bytes(&self) -> usize {
        self.hash
            .capacity()
            .saturating_mul(std::mem::size_of::<(K, T)>().saturating_add(16))
            .saturating_mul(2)
            .saturating_add(self.sorted.as_ref().map_or(0, |v| {
                v.capacity().saturating_mul(std::mem::size_of::<(K, T)>())
            }))
    }

    pub(super) fn new_budgeted() -> Self {
        Self {
            hash: HashMap::new(),
            sorted: Some(Vec::new()),
        }
    }
    #[cfg(test)]
    pub(super) fn capacity(&self) -> usize {
        self.sorted
            .as_ref()
            .map_or(self.hash.capacity(), Vec::capacity)
    }
    pub(super) fn len(&self) -> usize {
        self.sorted.as_ref().map_or(self.hash.len(), Vec::len)
    }
    pub(super) fn get(&self, code: &K) -> Option<&T> {
        if let Some(entries) = &self.sorted {
            entries
                .binary_search_by_key(code, |(key, _)| *key)
                .ok()
                .map(|index| &entries[index].1)
        } else {
            self.hash.get(code)
        }
    }
    pub(super) fn insert(&mut self, code: K, value: T) {
        if let Some(entries) = &mut self.sorted {
            match entries.binary_search_by_key(&code, |(key, _)| *key) {
                Ok(index) => entries[index].1 = value,
                Err(index) => entries.insert(index, (code, value)),
            }
        } else {
            self.hash.insert(code, value);
        }
    }
    fn iter(&self) -> impl Iterator<Item = (&K, &T)> {
        self.hash.iter().chain(
            self.sorted
                .iter()
                .flat_map(|entries| entries.iter().map(|(key, value)| (key, value))),
        )
    }
    fn into_entries(self) -> impl Iterator<Item = (K, T)> {
        self.hash
            .into_iter()
            .chain(self.sorted.into_iter().flatten())
    }
    fn try_reserve(&mut self, additional: usize) -> PdfResult<()> {
        if let Some(entries) = &mut self.sorted {
            entries
                .try_reserve(additional)
                .map_err(|_| PdfError::ResourceLimit("CMap mapping allocation".into()))
        } else {
            self.hash
                .try_reserve(additional)
                .map_err(|_| PdfError::ResourceLimit("CMap mapping allocation".into()))
        }
    }
    pub(super) fn reserve_with_budget(
        &mut self,
        additional: usize,
        pool: &crate::budget::OperationBudget,
        table: &mut Option<crate::budget::MemoryLease>,
    ) -> PdfResult<()> {
        let entries = self
            .sorted
            .as_mut()
            .expect("budget maps use sorted storage");
        let needed = entries
            .len()
            .checked_add(additional)
            .ok_or_else(|| PdfError::ResourceLimit("CMap mapping size".into()))?;
        if needed <= entries.capacity() {
            return Ok(());
        }
        let capacity = entries
            .capacity()
            .max(1)
            .checked_mul(2)
            .unwrap_or(needed)
            .max(needed);
        let slots = std::mem::size_of::<(K, T)>();
        let mut lease = pool.reserve(
            capacity
                .checked_mul(slots)
                .ok_or_else(|| PdfError::ResourceLimit("CMap mapping size".into()))?,
        )?;
        let mut next = Vec::new();
        next.try_reserve_exact(capacity)
            .map_err(|_| PdfError::ResourceLimit("CMap mapping allocation".into()))?;
        lease.resize(
            next.capacity()
                .checked_mul(slots)
                .ok_or_else(|| PdfError::ResourceLimit("CMap mapping capacity".into()))?,
        )?;
        next.append(entries);
        *entries = next;
        *table = Some(lease);
        Ok(())
    }
}

impl<T: PartialEq, K: Copy + Ord + std::hash::Hash> PartialEq for MappingTable<T, K> {
    fn eq(&self, other: &Self) -> bool {
        self.len() == other.len()
            && self
                .iter()
                .all(|(key, value)| other.get(key) == Some(value))
    }
}

#[derive(Debug, Clone, Default)]
pub struct CodeSpaceRange {
    size: u8,
    low: u32,
    high: u32,
}

impl CodeSpaceRange {
    pub fn new(low: u32, high: u32, size: u8) -> Self {
        Self { low, high, size }
    }

    pub fn is_contain_code(&self, code: u32) -> bool {
        self.low <= code && self.high >= code
    }

    pub fn size(&self) -> &u8 {
        &self.size
    }
}

#[derive(Debug, Clone, Default)]
pub struct CidRange {
    // Retained for the existing public record type; lookups use MappingTable.
    #[expect(
        dead_code,
        reason = "legacy public range record, not the lookup storage"
    )]
    start: u32,
    #[expect(
        dead_code,
        reason = "legacy public range record, not the lookup storage"
    )]
    end: u32,
    #[expect(
        dead_code,
        reason = "legacy public range record, not the lookup storage"
    )]
    start_cid: u32,
    #[expect(
        dead_code,
        reason = "legacy public range record, not the lookup storage"
    )]
    length: u8,
}

#[derive(Debug, Clone, Default)]
pub struct Cmap {
    wmode: Option<u8>,
    cmap_type: Option<u8>,
    code_space_range: Vec<CodeSpaceRange>,
    cid_to_unicode: MappingTable<String, (u8, u32)>,
    code_to_cids: MappingTable<u32, (u8, u32)>,
    // The fixed Adobe Identity-H/V programs cover every two-byte CID. Keep
    // that default arithmetic instead of constructing 65,536 hash entries.
    identity_cids: bool,
}

impl Cmap {
    pub(crate) fn cache_bytes(&self) -> usize {
        self.code_space_range
            .capacity()
            .saturating_mul(std::mem::size_of::<CodeSpaceRange>())
            .saturating_add(self.code_to_cids.storage_bytes())
            .saturating_add(self.cid_to_unicode.storage_bytes())
            .saturating_add(
                self.cid_to_unicode
                    .iter()
                    .map(|(_, s)| s.capacity())
                    .sum::<usize>(),
            )
    }

    /// Load a PDF CMap stream, including a named dictionary UseCMap base.
    /// Stream-valued dictionary bases remain unsupported rather than ignored.
    pub fn from_stream<R: std::io::Read + std::io::Seek>(
        stream: &crate::objects::pdf_stream::PdfStream,
        ctx: &crate::pdf_context::PDFContext<R>,
    ) -> PdfResult<Self> {
        Self::from_stream_inner(stream, ctx, None).map(|(cmap, _)| cmap)
    }

    /// Tracks resolution, decoded source, hexadecimal buffers and Unicode strings.
    /// Returns retained Unicode/code-space/mapping-table quota with the map.
    /// Inherited/array sources and inheritance names/slots are tracked; shared
    /// control blocks remain excluded. Resolver-internal allocations belong to the caller.
    pub(crate) fn from_stream_with_budget<R: std::io::Read + std::io::Seek>(
        stream: &crate::objects::pdf_stream::PdfStream,
        ctx: &crate::pdf_context::PDFContext<R>,
        budget: &crate::budget::OperationBudget,
    ) -> PdfResult<(Self, crate::budget::MemoryLease)> {
        let (cmap, lease) = Self::from_stream_inner(stream, ctx, Some(budget))?;
        Ok((cmap, lease.unwrap_or(budget.reserve(0)?)))
    }

    fn from_stream_inner<R: std::io::Read + std::io::Seek>(
        stream: &crate::objects::pdf_stream::PdfStream,
        ctx: &crate::pdf_context::PDFContext<R>,
        budget: Option<&crate::budget::OperationBudget>,
    ) -> PdfResult<(Self, Option<crate::budget::MemoryLease>)> {
        let resolve = |value: &crate::objects::PdfObject| {
            if let Some(pool) = budget {
                ctx.resolve_owned_with_budget(value, pool)
                    .map(|(value, lease)| (value, Some(lease)))
            } else {
                ctx.resolve(value).map(|value| (value, None))
            }
        };
        let dictionary_mode = stream
            .dict()
            .get("WMode")
            .map(|value| {
                let (value, _temporary) = resolve(value)?;
                value
                    .as_number()
                    .and_then(|number| number.as_u32_checked().ok())
                    .filter(|mode| *mode <= 1)
                    .map(|mode| mode as u8)
                    .ok_or_else(|| {
                        PdfError::FontError("CMap dictionary WMode must be 0 or 1".into())
                    })
            })
            .transpose()?;
        // Declare the lease before the parser so source bytes are dropped first.
        let (data, source_lease) = if let Some(pool) = budget {
            let (data, lease) = stream
                .decode_data_with_budget(ctx, pool)?
                .into_reserved_parts();
            (data, Some(lease))
        } else {
            (stream.decode_data(ctx)?, None)
        };
        let _source_lease = source_lease;
        let limits = ctx.limits();
        let mut parser = CmapParser::with_limits(data, &limits);
        if let Some(pool) = budget {
            parser.set_budget(pool);
        }
        let mut cmap = if let Some(base) = stream.dict().get("UseCMap") {
            let (base, _temporary) = resolve(base)?;
            let name = base.as_name().ok_or_else(|| {
                PdfError::UnsupportedFeature("Stream-valued dictionary UseCMap".into())
            })?;
            parser.parse_with_named_base(name.name())?
        } else {
            parser.parse()?
        };
        if let Some(mode) = dictionary_mode {
            if cmap.wmode.is_some_and(|program_mode| program_mode != mode) {
                return Err(PdfError::FontError(
                    "CMap dictionary and program WMode disagree".into(),
                ));
            }
            cmap.wmode = Some(mode);
        }
        Ok((cmap, parser.take_retained_lease()))
    }
    pub fn try_new(data: Vec<u8>) -> PdfResult<Self> {
        let mut parser = CmapParser::new(data);
        parser.parse()
    }

    pub fn try_new_with_limits(
        data: Vec<u8>,
        limits: &crate::parser::options::Limits,
    ) -> PdfResult<Self> {
        CmapParser::with_limits(data, limits).parse()
    }

    pub fn new_from_predefined(name: &str) -> PdfResult<Self> {
        Self::new_from_predefined_with_limits(name, &crate::parser::options::Limits::default())
    }

    pub(crate) fn new_from_predefined_with_budget(
        name: &str,
        limits: &crate::parser::options::Limits,
        budget: &crate::budget::OperationBudget,
    ) -> PdfResult<(Self, crate::budget::MemoryLease)> {
        let data = get_predefine_cmap_data(name)
            .ok_or_else(|| PdfError::FontError(format!("predefine cmap {name:?} not found")))?;
        if data.len() > limits.max_decoded_bytes {
            return Err(PdfError::ResourceLimit(
                "Predefined CMap source bytes".into(),
            ));
        }
        let (data, _source_lease) =
            crate::budget::BudgetedBytes::copy_from_slice(budget, data)?.into_reserved_parts();
        let mut parser = CmapParser::with_limits(data, limits);
        parser.set_budget(budget);
        let cmap = parser.parse()?;
        let lease = parser.take_retained_lease().unwrap_or(budget.reserve(0)?);
        Ok((cmap, lease))
    }

    /// Parse with caller-provided named base programs, falling back to built-ins
    /// when the resolver returns None. Returned source bytes are charged to the
    /// shared budget; allocations performed inside the resolver are its responsibility.
    pub fn try_new_with_resolver(
        data: Vec<u8>,
        limits: &crate::parser::options::Limits,
        resolver: &dyn Fn(&str) -> PdfResult<Option<Vec<u8>>>,
    ) -> PdfResult<Self> {
        let mut parser = CmapParser::with_limits(data, limits);
        parser.set_resolver(resolver);
        parser.parse()
    }

    pub fn new_from_predefined_with_limits(
        name: &str,
        limits: &crate::parser::options::Limits,
    ) -> PdfResult<Self> {
        let cmap_data = get_predefine_cmap_data(name).ok_or(PdfError::FontError(format!(
            "predefine cmap {:?} not found",
            name
        )))?;
        if cmap_data.len() > limits.max_decoded_bytes {
            return Err(PdfError::ResourceLimit(
                "Predefined CMap source bytes".into(),
            ));
        }
        if matches!(name, "Identity-H" | "Identity-V") {
            let vertical = name == "Identity-V";
            let source_bytes = cmap_data.len().saturating_add(if vertical {
                get_predefine_cmap_data("Identity-H")
                    .ok_or_else(|| PdfError::FontError("bundled Identity-H missing".into()))?
                    .len()
            } else {
                0
            });
            // Preserve the expanded parser's logical entry/source/depth caps.
            if limits.max_objects < 65_537 {
                return Err(PdfError::ResourceLimit("CMap mapping entries".into()));
            }
            if source_bytes > limits.max_decoded_bytes {
                return Err(PdfError::ResourceLimit(
                    "CMap cumulative source bytes".into(),
                ));
            }
            if vertical && limits.max_depth == 0 {
                return Err(PdfError::ResourceLimit("CMap inheritance depth".into()));
            }
            let mut spaces = Vec::new();
            spaces
                .try_reserve_exact(1)
                .map_err(|_| PdfError::ResourceLimit("CMap spaces allocation".into()))?;
            spaces.push(CodeSpaceRange::new(0, 65_535, 2));
            return Ok(Self {
                wmode: Some(u8::from(vertical)),
                identity_cids: true,
                code_space_range: spaces,
                ..Default::default()
            });
        }
        let mut data = Vec::new();
        data.try_reserve_exact(cmap_data.len())
            .map_err(|_| PdfError::ResourceLimit("Predefined CMap allocation".into()))?;
        data.extend_from_slice(cmap_data);
        let cmap = Cmap::try_new_with_limits(data, limits)?;
        Ok(cmap)
    }

    pub fn usecmap(&mut self, other: Cmap) {
        self.code_space_range = other.code_space_range;
        self.cid_to_unicode = other.cid_to_unicode;
        self.code_to_cids = other.code_to_cids;
        self.identity_cids = other.identity_cids;
    }

    pub fn add_code_space_range(&mut self, space_range: CodeSpaceRange) {
        self.code_space_range.push(space_range);
    }

    fn inherit_defaults(&mut self, other: Cmap) -> PdfResult<()> {
        self.code_space_range
            .try_reserve(other.code_space_range.len())
            .map_err(|_| PdfError::ResourceLimit("Inherited CMap spaces allocation".into()))?;
        self.cid_to_unicode
            .try_reserve(other.cid_to_unicode.len())
            .map_err(|_| PdfError::ResourceLimit("Inherited CMap Unicode allocation".into()))?;
        self.code_to_cids
            .try_reserve(other.code_to_cids.len())
            .map_err(|_| PdfError::ResourceLimit("Inherited CMap CID allocation".into()))?;
        self.code_space_range.extend(other.code_space_range);
        for (key, value) in other.cid_to_unicode.into_entries() {
            if self.cid_to_unicode.get(&key).is_none() {
                self.cid_to_unicode.insert(key, value);
            }
        }
        for (key, value) in other.code_to_cids.into_entries() {
            if self.exact_cid(key).is_none() {
                self.code_to_cids.insert(key, value);
            }
        }
        self.identity_cids |= other.identity_cids;
        if self.wmode.is_none() {
            self.wmode = other.wmode;
        }
        if self.cmap_type.is_none() {
            self.cmap_type = other.cmap_type;
        }
        Ok(())
    }

    pub fn add_unicode(&mut self, src_code: u32, unicode: String) {
        // Legacy programmatic mappings have no encoded length. Parsed maps
        // always store the exact source length instead.
        self.cid_to_unicode.insert((0, src_code), unicode);
    }

    pub fn unicode(&self, char: &CharCode) -> Option<&str> {
        if !self.admits_unicode_code(char.code, char.length) {
            return None;
        }
        self.cid_to_unicode
            .get(&(char.length, char.code))
            .or_else(|| self.cid_to_unicode.get(&(0, char.code)))
            .map(String::as_str)
    }

    /// A simple font always supplies one-byte codes independently of a CMap's
    /// codespace declaration. Honour an exact one-byte mapping even when a
    /// producer copied a two-byte Identity codespace into its ToUnicode header.
    /// Never reinterpret a two-byte source entry as a one-byte code.
    pub(crate) fn single_byte_unicode(&self, code: u8) -> Option<&str> {
        self.cid_to_unicode
            .get(&(1, u32::from(code)))
            .or_else(|| self.cid_to_unicode.get(&(0, u32::from(code))))
            .map(String::as_str)
    }

    pub fn add_code_to_cid(&mut self, code: u32, cid: u32) {
        self.code_to_cids.insert((0, code), cid);
    }

    /// Resolve an encoding character code to a CID; unmapped codes select .notdef.
    pub fn cid(&self, code: u32) -> u32 {
        // Preserve the lengthless inspection API only for unambiguous values.
        let mut result = None;
        for length in 0..=4 {
            if let Some(value) = self.exact_cid((length, code)) {
                if result.is_some_and(|previous| previous != value) {
                    return 0;
                }
                result = Some(value);
            }
        }
        result.unwrap_or(0)
    }

    /// Encoding lookup preserves source byte length; an unmapped code is .notdef.
    pub fn cid_for_char(&self, ch: &CharCode) -> u32 {
        self.exact_cid((ch.length, ch.code))
            .or_else(|| self.exact_cid((0, ch.code)))
            .unwrap_or(0)
    }

    fn exact_cid(&self, key: (u8, u32)) -> Option<u32> {
        self.code_to_cids
            .get(&key)
            .copied()
            .or_else(|| (self.identity_cids && key.0 == 2 && key.1 <= 65_535).then_some(key.1))
    }

    pub(crate) fn unicode_entries(&self) -> impl Iterator<Item = (u32, u8, &str)> {
        self.cid_to_unicode
            .iter()
            .map(|((length, code), text)| (*code, *length, text.as_str()))
    }

    pub(crate) fn admits_unicode_code(&self, code: u32, length: u8) -> bool {
        self.code_space_range.is_empty()
            || self
                .code_space_range
                .iter()
                .any(|range| range.size == length && range.is_contain_code(code))
    }

    pub fn wmode(&self) -> Option<u8> {
        self.wmode
    }

    fn find_charsize(&self, bytes: &[u8]) -> Option<u8> {
        let code = bytes_to_u32(bytes);
        let size = bytes.len() as u8;
        for range in &self.code_space_range {
            if range.size != size {
                continue;
            }
            if range.is_contain_code(code) {
                return Some(range.size);
            }
        }
        None
    }

    pub fn next_char(&self, bytes: &[u8], offset: usize) -> Option<CharCode> {
        if offset >= bytes.len() {
            return None;
        }
        // Borrow each candidate (at most four bytes); avoid a heap allocation
        // for every decoded character and offset arithmetic past the input.
        for length in 1..=4.min(bytes.len() - offset) {
            let codecs = &bytes[offset..offset + length];
            if let Some(v) = self.find_charsize(codecs) {
                let ch = CharCode::new(bytes_to_u32(codecs), v, 0.0);
                return Some(ch);
            }
        }
        None
    }

    pub fn chars(&self, bytes: &[u8]) -> PdfResult<Vec<CharCode>> {
        self.chars_limited(bytes, usize::MAX)
    }

    pub fn chars_limited(&self, bytes: &[u8], limit: usize) -> PdfResult<Vec<CharCode>> {
        let mut res = Vec::new();
        self.visit_chars_limited(bytes, limit, |character| {
            res.try_reserve(1)
                .map_err(|_| PdfError::ResourceLimit("CMap character allocation".into()))?;
            res.push(character);
            Ok(())
        })?;
        Ok(res)
    }

    /// Visit decoded codes without retaining a character vector. Stop before
    /// exceeding the count limit, and propagate visitor failures immediately.
    pub(crate) fn visit_chars_limited(
        &self,
        bytes: &[u8],
        limit: usize,
        mut visitor: impl FnMut(CharCode) -> PdfResult<()>,
    ) -> PdfResult<usize> {
        let mut offset = 0;
        let mut count = 0;
        while offset < bytes.len() {
            if count >= limit {
                return Err(PdfError::ResourceLimit("decoded CMap characters".into()));
            }
            if let Some(charcode) = self.next_char(bytes, offset) {
                offset = offset
                    .checked_add(usize::from(charcode.length()))
                    .filter(|&next| next > offset && next <= bytes.len())
                    .ok_or_else(|| {
                        PdfError::FontError("CMap character made invalid progress".into())
                    })?;
                visitor(charcode)?;
                count += 1;
            } else {
                return Err(PdfError::FontError(format!(
                    "invalid or truncated CMap character at byte offset {offset}"
                )));
            }
        }
        Ok(count)
    }
}

#[cfg(test)]
mod compact_identity_tests {
    use super::*;
    #[test]
    fn compact_identity_matches_every_expanded_cid_and_keeps_length_and_overrides() {
        let limits = crate::parser::options::Limits::default();
        for name in ["Identity-H", "Identity-V"] {
            let mut compact = Cmap::new_from_predefined_with_limits(name, &limits).unwrap();
            let mut expanded =
                CmapParser::with_limits(get_predefine_cmap_data(name).unwrap().to_vec(), &limits)
                    .parse()
                    .unwrap();
            assert!(compact.cache_bytes() < 1024);
            assert!(expanded.cache_bytes() > 1024 * 1024);
            assert_eq!(compact.wmode(), expanded.wmode());
            for code in 0..=65_535 {
                assert_eq!(compact.cid(code), expanded.cid(code));
                for length in [1, 2, 3] {
                    let ch = CharCode::new(code, length, 0.);
                    assert_eq!(compact.cid_for_char(&ch), expanded.cid_for_char(&ch));
                }
            }
            for code in [1, 32, 65535] {
                compact.add_code_to_cid(code, 42);
                expanded.add_code_to_cid(code, 42);
                assert_eq!(compact.cid(code), expanded.cid(code));
                for length in [1, 2] {
                    let ch = CharCode::new(code, length, 0.);
                    assert_eq!(compact.cid_for_char(&ch), expanded.cid_for_char(&ch));
                }
            }
            assert!(compact.chars_limited(&[0], 1).is_err());
            assert_eq!(compact.cid_for_char(&CharCode::new(65536, 2, 0.)), 0);
        }
    }
    #[test]
    fn compact_defaults_preserve_local_overrides_and_base_precedence() {
        let mut local = Cmap::default();
        local.code_to_cids.insert((2, 1), 42);
        local
            .inherit_defaults(Cmap::new_from_predefined("Identity-V").unwrap())
            .unwrap();
        assert_eq!(local.cid_for_char(&CharCode::new(1, 2, 0.)), 42);
        assert_eq!(local.cid_for_char(&CharCode::new(2, 2, 0.)), 2);
        let mut later_base = Cmap::default();
        later_base.code_to_cids.insert((2, 2), 99);
        local.inherit_defaults(later_base).unwrap();
        assert_eq!(local.cid_for_char(&CharCode::new(2, 2, 0.)), 2);
        assert_eq!(local.wmode(), Some(1));
        let mut swapped = Cmap::default();
        swapped.usecmap(local);
        // The legacy replacement API preserves its receiver's writing mode.
        assert_eq!(swapped.wmode(), None);
        assert_eq!(swapped.cid_for_char(&CharCode::new(1, 2, 0.)), 42);
        assert_eq!(swapped.cid_for_char(&CharCode::new(2, 2, 0.)), 2);
    }
    #[test]
    fn compact_factory_preserves_expanded_source_depth_and_logical_entry_limits() {
        let h = get_predefine_cmap_data("Identity-H").unwrap().len();
        let v = get_predefine_cmap_data("Identity-V").unwrap().len();
        for name in ["Identity-H", "Identity-V"] {
            for max_objects in [1, 65536, 65537] {
                for max_depth in [0, 1] {
                    for max_decoded_bytes in [h - 1, h, h + v - 1, h + v] {
                        let limits = crate::parser::options::Limits {
                            max_objects,
                            max_depth,
                            max_decoded_bytes,
                            ..Default::default()
                        };
                        let ordinary = CmapParser::with_limits(
                            get_predefine_cmap_data(name).unwrap().to_vec(),
                            &limits,
                        )
                        .parse();
                        let compact = Cmap::new_from_predefined_with_limits(name, &limits);
                        assert_eq!(
                            compact.is_ok(),
                            ordinary.is_ok(),
                            "{name} objects={max_objects} depth={max_depth} bytes={max_decoded_bytes}"
                        );
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn sorted_mapping_budget_tracks_capacity_duplicates_and_retry() {
        use crate::budget::OperationBudget;
        let run = |pool: &OperationBudget| {
            let mut table = super::MappingTable::<u32>::new_budgeted();
            let mut lease = None;
            for (key, value) in [(u32::MAX, 9), (5, 10), (0, 11), (5, 12)] {
                if table.get(&key).is_none() {
                    table.reserve_with_budget(1, pool, &mut lease)?;
                }
                table.insert(key, value);
            }
            crate::error::PdfResult::Ok((table, lease.unwrap()))
        };
        let slots = std::mem::size_of::<(u32, u32)>();
        let pool = OperationBudget::new(6 * slots);
        let (table, lease) = run(&pool).unwrap();
        assert!(table.hash.is_empty());
        assert_eq!(table.len(), 3);
        assert_eq!(table.get(&5), Some(&12));
        assert_eq!(table.get(&u32::MAX), Some(&9));
        assert_eq!(
            table.iter().map(|(key, _)| *key).collect::<Vec<_>>(),
            [0, 5, u32::MAX]
        );
        assert_eq!(
            lease.bytes(),
            table.sorted.as_ref().unwrap().capacity() * slots
        );
        assert_eq!(pool.live_bytes(), 4 * slots);
        assert_eq!(pool.peak_bytes(), 6 * slots);
        drop((table, lease));
        assert_eq!(pool.live_bytes(), 0);
        let neighbor = pool.reserve(1).unwrap();
        assert!(run(&pool).is_err());
        assert_eq!(pool.live_bytes(), 1);
        drop(neighbor);
        drop(run(&pool).unwrap());
        assert_eq!(pool.live_bytes(), 0);
    }

    #[test]
    fn budget_stream_source_releases_on_success_mode_error_and_limit_recovery() {
        use crate::{
            budget::OperationBudget,
            document::owned::Document,
            objects::{
                PdfObject, pdf_dict::PdfDict, pdf_number::PdfNumber, pdf_reference::PdfReference,
                pdf_stream::PdfStream,
            },
        };
        let doc = Document::new().unwrap();
        let mode = doc
            .try_insert_object(PdfObject::PdfNumber(PdfNumber::new(0.0)))
            .unwrap();
        let mut dict = PdfDict::default();
        dict.insert(
            "WMode".into(),
            PdfObject::PdfReference(PdfReference::from_id(mode)),
        );
        let stream = PdfStream::new(
            dict.clone(),
            b"/WMode 0 def 1 beginbfchar <41> <0041> endbfchar".to_vec(),
        );
        let probe = OperationBudget::new(65536);
        let (cmap, lease) =
            super::Cmap::from_stream_with_budget(&stream, doc.ctx(), &probe).unwrap();
        assert_eq!(
            cmap.unicode(&crate::font::CharCode::new(65, 1, 0.0)),
            Some("A")
        );
        // Unicode strings are retained; mapping table buckets remain a TODO.
        assert_eq!(probe.live_bytes(), lease.bytes());
        assert!(lease.bytes() > 0);
        let peak = probe.peak_bytes();
        drop((cmap, lease));
        assert_eq!(probe.live_bytes(), 0);
        assert!(peak >= stream.raw_data().len());
        let pool = OperationBudget::new(peak);
        let neighbor = pool.reserve(1).unwrap();
        assert!(matches!(
            super::Cmap::from_stream_with_budget(&stream, doc.ctx(), &pool),
            Err(crate::error::PdfError::ResourceLimit(_))
        ));
        assert_eq!(pool.live_bytes(), 1);
        drop(neighbor);
        assert!(super::Cmap::from_stream_with_budget(&stream, doc.ctx(), &pool).is_ok());
        assert_eq!(pool.live_bytes(), 0);
        doc.update_object(mode, PdfObject::PdfNumber(PdfNumber::new(1.0)))
            .unwrap();
        assert!(super::Cmap::from_stream_with_budget(&stream, doc.ctx(), &pool).is_err());
        assert_eq!(pool.live_bytes(), 0);
        let invalid = PdfStream::new(dict, b"1 beginbfchar <41>".to_vec());
        assert!(super::Cmap::from_stream_with_budget(&invalid, doc.ctx(), &pool).is_err());
        assert_eq!(pool.live_bytes(), 0);
    }

    #[test]
    fn cmap_visitor_stops_at_limit_or_callback_failure() {
        use crate::error::PdfError;
        let cmap =
            super::Cmap::try_new(b"1 begincodespacerange <00> <FF> endcodespacerange".to_vec())
                .unwrap();
        let mut visited = 0;
        assert!(matches!(
            cmap.visit_chars_limited(&[1, 2, 3], 2, |_| {
                visited += 1;
                Ok(())
            }),
            Err(PdfError::ResourceLimit(_))
        ));
        assert_eq!(visited, 2);
        visited = 0;
        assert!(matches!(
            cmap.visit_chars_limited(&[1, 2, 3], 3, |_| {
                visited += 1;
                Err(PdfError::ObjectError("visitor failure".into()))
            }),
            Err(PdfError::ObjectError(_))
        ));
        assert_eq!(visited, 1);
        assert_eq!(
            cmap.visit_chars_limited(&[], 0, |_| panic!("empty input must not visit"))
                .unwrap(),
            0
        );
        assert_eq!(cmap.visit_chars_limited(&[1, 2], 2, |_| Ok(())).unwrap(), 2);
    }

    #[test]
    fn borrowed_next_character_handles_all_lengths_and_invalid_offsets() {
        let cmap = super::Cmap::try_new(b"4 begincodespacerange <21> <7F> <8000> <80FF> <810000> <81FFFF> <82000000> <82FFFFFF> endcodespacerange".to_vec()).unwrap();
        let bytes = [0x21, 0x80, 0x01, 0x81, 0, 1, 0x82, 0, 0, 1];
        for (offset, value, length) in [
            (0, 0x21, 1),
            (1, 0x8001, 2),
            (3, 0x810001, 3),
            (6, 0x82000001, 4),
        ] {
            let character = cmap.next_char(&bytes, offset).unwrap();
            assert_eq!((character.code(), character.length()), (value, length));
            assert!(
                cmap.next_char(&bytes[..offset + usize::from(length) - 1], offset)
                    .is_none()
            );
        }
        for offset in [bytes.len(), bytes.len() + 1, usize::MAX] {
            assert!(cmap.next_char(&bytes, offset).is_none());
        }
        assert!(cmap.next_char(&[], 0).is_none());
        assert!(cmap.next_char(&[0], 0).is_none());
    }

    #[test]
    fn stream_dictionary_wmode_is_validated_and_applied() {
        use crate::{
            objects::{PdfObject, pdf_dict::PdfDict, pdf_number::PdfNumber, pdf_stream::PdfStream},
            parser::options::ParseOptions,
            pdf_context::PDFContext,
        };
        let ctx = PDFContext::<std::io::Cursor<Vec<u8>>>::new_without_source(
            PdfDict::default(),
            ParseOptions::default(),
        );
        for mode in [0.0, 1.0] {
            let mut dict = PdfDict::default();
            dict.try_insert("WMode".into(), PdfObject::PdfNumber(PdfNumber::new(mode)))
                .unwrap();
            let cmap =
                super::Cmap::from_stream(&PdfStream::new(dict.clone(), Vec::new()), &ctx).unwrap();
            assert_eq!(cmap.wmode(), Some(mode as u8));
            let program = format!("/WMode {} def", 1 - mode as u8).into_bytes();
            assert!(super::Cmap::from_stream(&PdfStream::new(dict, program), &ctx).is_err());
        }
        for mode in [-1.0, 2.0, 0.5] {
            let mut dict = PdfDict::default();
            dict.try_insert("WMode".into(), PdfObject::PdfNumber(PdfNumber::new(mode)))
                .unwrap();
            assert!(super::Cmap::from_stream(&PdfStream::new(dict, Vec::new()), &ctx).is_err());
        }
    }

    use super::{Cmap, CodeSpaceRange};

    #[test]
    fn custom_base_resolver_inherits_and_detects_real_cycles() {
        let limits = crate::parser::options::Limits::default();
        let resolver = |name: &str| {
            Ok(match name {
            "Base" => Some(b"1 begincodespacerange <00> <FF> endcodespacerange 1 beginbfchar <01> <0041> endbfchar".to_vec()),
            "CycleA" => Some(b"/CycleB usecmap".to_vec()),
            "CycleB" => Some(b"/CycleA usecmap".to_vec()),
            _ => None,
        })
        };
        let cmap = Cmap::try_new_with_resolver(
            b"/Base usecmap 1 beginbfchar <01> <0042> endbfchar".to_vec(),
            &limits,
            &resolver,
        )
        .unwrap();
        let chars = cmap.chars_limited(&[1], 1).unwrap();
        assert_eq!(cmap.unicode(&chars[0]), Some("B"));
        assert!(
            matches!(Cmap::try_new_with_resolver(b"/CycleA usecmap".to_vec(), &limits, &resolver),
            Err(crate::error::PdfError::FontError(message)) if message.contains("cycle"))
        );
    }

    #[test]
    fn resolver_failure_and_oversized_source_propagate() {
        let data = b"/Base usecmap".to_vec();
        let limits = crate::parser::options::Limits {
            max_decoded_bytes: data.len() + 1,
            ..Default::default()
        };
        let oversized = |_: &str| Ok(Some(vec![b' '; 2]));
        assert!(matches!(
            Cmap::try_new_with_resolver(data.clone(), &limits, &oversized),
            Err(crate::error::PdfError::ResourceLimit(_))
        ));
        let failure = |_: &str| Err(crate::error::PdfError::FontError("resolver failed".into()));
        assert!(
            matches!(Cmap::try_new_with_resolver(data, &limits, &failure),
            Err(crate::error::PdfError::FontError(message)) if message == "resolver failed")
        );
    }

    #[test]
    fn identity_vertical_mapping_retains_budget_and_code_lengths() {
        let limits = crate::parser::options::Limits::default();
        let cmap = Cmap::new_from_predefined_with_limits("Identity-V", &limits).unwrap();
        assert_eq!(cmap.wmode(), Some(1));
        assert!(
            Cmap::new_from_predefined_with_limits(
                "Identity-V",
                &crate::parser::options::Limits {
                    max_depth: 0,
                    ..limits
                }
            )
            .is_err()
        );
        let chars = cmap.chars_limited(&[0, 1, 255, 255], 2).unwrap();
        assert_eq!(
            chars
                .iter()
                .map(|ch| (ch.code(), ch.length()))
                .collect::<Vec<_>>(),
            vec![(1, 2), (65535, 2)]
        );
        assert!(cmap.chars_limited(&[0, 1, 255, 255], 1).is_err());
        assert!(cmap.chars_limited(&[0], 1).is_err());
        assert!(
            Cmap::new_from_predefined_with_limits(
                "Identity-V",
                &crate::parser::options::Limits {
                    max_objects: 1,
                    ..limits
                }
            )
            .is_err()
        );
    }

    #[test]
    fn predefined_cmaps_obey_custom_mapping_limit() {
        let size = font_data::cmap::get_predefine_cmap_data("Identity-H")
            .unwrap()
            .len();
        for (budget, success) in [(size, true), (size - 1, false)] {
            let limits = crate::parser::options::Limits {
                max_decoded_bytes: budget,
                ..Default::default()
            };
            assert_eq!(
                Cmap::new_from_predefined_with_limits("Identity-H", &limits).is_ok(),
                success
            );
        }
        let limits = crate::parser::options::Limits {
            max_objects: 1,
            ..Default::default()
        };
        assert!(matches!(
            Cmap::new_from_predefined_with_limits("Identity-H", &limits),
            Err(crate::error::PdfError::ResourceLimit(_))
        ));
        assert!(Cmap::new_from_predefined("Identity-H").is_ok());
    }

    #[test]
    fn invalid_and_truncated_codes_return_errors() {
        let mut cmap = Cmap::default();
        cmap.add_code_space_range(CodeSpaceRange::new(0x8100, 0x81ff, 2));
        assert!(cmap.chars(&[0x80, 0x00]).is_err());
        assert!(cmap.chars(&[0x81]).is_err());
        assert!(cmap.next_char(&[], 0).is_none());
        assert!(cmap.next_char(&[0x81], 1).is_none());
        assert!(cmap.chars(&[]).unwrap().is_empty());
    }

    #[test]
    fn variable_length_codes_preserve_order() {
        let mut cmap = Cmap::default();
        cmap.add_code_space_range(CodeSpaceRange::new(0x20, 0x7e, 1));
        cmap.add_code_space_range(CodeSpaceRange::new(0x8100, 0x81ff, 2));
        cmap.add_code_space_range(CodeSpaceRange::new(0x900000, 0x90ffff, 3));
        cmap.add_code_space_range(CodeSpaceRange::new(0xa0000000, 0xa0ffffff, 4));
        let chars = cmap
            .chars(&[0x41, 0x81, 0x02, 0x90, 0x00, 0x03, 0xa0, 0x00, 0x00, 0x04])
            .unwrap();
        assert_eq!(
            chars
                .iter()
                .map(|ch| (ch.code(), ch.length()))
                .collect::<Vec<_>>(),
            vec![(0x41, 1), (0x8102, 2), (0x900003, 3), (0xa0000004, 4)]
        );
        assert_eq!(cmap.chars_limited(&[0x81, 0x02], 1).unwrap().len(), 1);
        assert!(cmap.chars_limited(&[0x41, 0x81, 0x02], 1).is_err());
        assert!(cmap.chars_limited(&[0x41], 0).is_err());
        assert!(cmap.chars_limited(&[], 0).unwrap().is_empty());
    }
}

fn bytes_to_u32(bytes: &[u8]) -> u32 {
    let mut res = 0;
    for v in bytes {
        res = (res << 8) + v.to_owned() as u32;
    }
    res
}

#[cfg(test)]
mod licensed_cmap_regressions {
    use super::Cmap;
    use md5::{Digest, Md5};

    #[test]
    fn adobe_replacement_preserves_effective_cids_and_inherited_encodings() {
        // Captured before replacement, independently checked against Adobe
        // release 20190730. MD5 is a regression fingerprint, not a trust check.
        type Case = (
            &'static str,
            usize,
            &'static str,
            Option<u8>,
            &'static [(u8, u32, u32)],
        );
        let cases: &[Case] = &[
            (
                "GBK-EUC-H",
                22118,
                "e8c1338e093e6aa1cdee3a4b72005a81",
                Some(0),
                &[(1, 0, 128), (2, 33088, 65278)],
            ),
            (
                "GBK-EUC-V",
                22118,
                "87af218234e447ec64faa12a63b8b1e5",
                Some(1),
                &[(1, 0, 128), (2, 33088, 65278)],
            ),
            (
                "GBK2K-H",
                30224,
                "4658e7f68722bbd544c8e6c9f3bb31b1",
                Some(0),
                &[(1, 0, 127), (2, 33088, 65278), (4, 2167439664, 4265213497)],
            ),
            (
                "GBK2K-V",
                30224,
                "338e93e85e571e85d461d26d8a258f31",
                Some(1),
                &[(1, 0, 127), (2, 33088, 65278), (4, 2167439664, 4265213497)],
            ),
            (
                "GBKp-EUC-H",
                22118,
                "37764fd9ffa87263761d5dc85af95524",
                Some(0),
                &[(1, 0, 128), (2, 33088, 65278)],
            ),
            (
                "GBKp-EUC-V",
                22118,
                "4bf1b33f11670d3906bd5369139d4c40",
                Some(1),
                &[(1, 0, 128), (2, 33088, 65278)],
            ),
            (
                "UniCNS-UCS2-H",
                18285,
                "21c3c3db9da67a58970414b742cb0d49",
                Some(0),
                &[(2, 0, 55295), (2, 57344, 65535)],
            ),
            (
                "UniCNS-UCS2-V",
                18285,
                "2fe07fbd339754fb3892051a00ac6ce4",
                Some(1),
                &[(2, 0, 55295), (2, 57344, 65535)],
            ),
            (
                "UniCNS-UTF16-H",
                23715,
                "0fb4a7ca40c918a92c3139bd63c57f50",
                Some(0),
                &[
                    (2, 0, 55295),
                    (2, 57344, 65535),
                    (4, 3623934976, 3690979327),
                ],
            ),
            (
                "UniCNS-UTF16-V",
                23715,
                "db91ec62b1a16ced5bfb616e2bf47958",
                Some(1),
                &[
                    (2, 0, 55295),
                    (2, 57344, 65535),
                    (4, 3623934976, 3690979327),
                ],
            ),
            (
                "UniGB-UCS2-H",
                28840,
                "136f847ccdd0bae47a56f9c6af88e009",
                Some(0),
                &[(2, 0, 55295), (2, 57344, 65535)],
            ),
            (
                "UniGB-UCS2-V",
                28840,
                "6e8f1a6a4a7fd6aedde876ce658640a3",
                Some(1),
                &[(2, 0, 55295), (2, 57344, 65535)],
            ),
            (
                "UniGB-UTF16-H",
                30217,
                "2ef44db33c7ee8c48c2618982f497d2b",
                Some(0),
                &[
                    (2, 0, 55295),
                    (2, 57344, 65535),
                    (4, 3623934976, 3690979327),
                ],
            ),
            (
                "UniGB-UTF16-V",
                30217,
                "4e1e3f289cf96d214ee2aab0b2650d20",
                Some(1),
                &[
                    (2, 0, 55295),
                    (2, 57344, 65535),
                    (4, 3623934976, 3690979327),
                ],
            ),
            (
                "UniJIS-UCS2-H",
                9772,
                "1d60625dae74ea77c513ff43293e44e9",
                Some(0),
                &[(2, 0, 55295), (2, 57344, 65535)],
            ),
            (
                "UniJIS-UCS2-HW-H",
                9772,
                "b3696a09497538b3d62f4611d4ec5aef",
                Some(0),
                &[(2, 0, 55295), (2, 57344, 65535)],
            ),
            (
                "UniJIS-UCS2-HW-V",
                9772,
                "117deb4ba1dbac42c041ba6c2b31c8f9",
                Some(1),
                &[(2, 0, 55295), (2, 57344, 65535)],
            ),
            (
                "UniJIS-UCS2-V",
                9772,
                "c1f05ec90524fbc28ad3ccec6b5130d0",
                Some(1),
                &[(2, 0, 55295), (2, 57344, 65535)],
            ),
            (
                "UniJIS-UTF16-H",
                15889,
                "c0d7277a5dc59531e80f9d632e88918f",
                Some(0),
                &[
                    (2, 0, 55295),
                    (2, 57344, 65535),
                    (4, 3623934976, 3690979327),
                ],
            ),
            (
                "UniJIS-UTF16-V",
                15889,
                "009634b57bc56250359c37837fbb2d78",
                Some(1),
                &[
                    (2, 0, 55295),
                    (2, 57344, 65535),
                    (4, 3623934976, 3690979327),
                ],
            ),
            (
                "UniKS-UCS2-H",
                17326,
                "e43678376bdcd75098fe8dc0f824d3f3",
                Some(0),
                &[(2, 0, 55295), (2, 57344, 65535)],
            ),
            (
                "UniKS-UCS2-V",
                17326,
                "a606142b63043155aa5fdc0a807f71fb",
                Some(1),
                &[(2, 0, 55295), (2, 57344, 65535)],
            ),
            (
                "UniKS-UTF16-H",
                17488,
                "579d45b508c921460d02dac0affc4d8c",
                Some(0),
                &[
                    (2, 0, 55295),
                    (2, 57344, 65535),
                    (4, 3623934976, 3690979327),
                ],
            ),
            (
                "UniKS-UTF16-V",
                17488,
                "a760418afe4e4dbf03fe56e20874127b",
                Some(1),
                &[
                    (2, 0, 55295),
                    (2, 57344, 65535),
                    (4, 3623934976, 3690979327),
                ],
            ),
        ];
        for &(name, count, expected_md5, wmode, spaces) in cases {
            let cmap = Cmap::new_from_predefined(name).unwrap();
            assert_eq!(cmap.wmode(), wmode, "{name}");
            let mut actual_spaces: Vec<_> = cmap
                .code_space_range
                .iter()
                .map(|s| (s.size, s.low, s.high))
                .collect();
            actual_spaces.sort_unstable();
            assert_eq!(actual_spaces, spaces, "{name}");
            let mut entries: Vec<_> = cmap
                .code_to_cids
                .iter()
                .filter(|(code, cid)| {
                    **cid != 0
                        && cmap
                            .code_space_range
                            .iter()
                            .any(|s| s.size == code.0 && s.is_contain_code(code.1))
                })
                .map(|(code, cid)| (code.1, *cid))
                .collect();
            entries.sort_unstable();
            assert_eq!(entries.len(), count, "{name}");
            let mut digest = Md5::new();
            for (code, cid) in entries {
                digest.update(code.to_be_bytes());
                digest.update(cid.to_be_bytes());
            }
            assert_eq!(format!("{:x}", digest.finalize()), expected_md5, "{name}");
        }
        for name in ["GBK-X", "UniCNS-X", "UniGB-X", "UniJIS-X", "UniKS-X"] {
            assert!(font_data::cmap::get_predefine_cmap_data(name).is_none());
            assert!(Cmap::new_from_predefined(name).is_err());
        }
    }
}
