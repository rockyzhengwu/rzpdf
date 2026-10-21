use crate::error::{PdfError, PdfResult};
use crate::font::cmap::{Cmap, CodeSpaceRange};
use crate::parser::character::{is_delimiter, is_end_of_line, is_white_space};
use crate::parser::parse_utility::hex_to_u8;
use std::rc::Rc;

#[derive(Debug)]
struct HexBytes {
    data: Vec<u8>,
    _lease: Option<crate::budget::MemoryLease>,
}

impl std::ops::Deref for HexBytes {
    type Target = Vec<u8>;
    fn deref(&self) -> &Self::Target {
        &self.data
    }
}

impl std::ops::DerefMut for HexBytes {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.data
    }
}

/// A token keeps the source alive without allocating or copying token bytes.
struct Word {
    source: Rc<Vec<u8>>,
    start: usize,
    end: usize,
}

impl Word {
    fn as_slice(&self) -> &[u8] {
        &self.source[self.start..self.end]
    }
}

impl std::ops::Deref for Word {
    type Target = [u8];
    fn deref(&self) -> &Self::Target {
        self.as_slice()
    }
}

fn decode_utf16be(bytes: &[u8]) -> String {
    let (cs, _enc, _has_err) = encoding_rs::UTF_16BE.decode(bytes);
    cs.into_owned()
}

fn decode_utf16be_with_budget(
    bytes: &[u8],
    budget: &crate::budget::OperationBudget,
) -> PdfResult<(String, crate::budget::MemoryLease)> {
    let mut decoder = encoding_rs::UTF_16BE.new_decoder();
    let capacity = decoder
        .max_utf8_buffer_length(bytes.len())
        .ok_or_else(|| PdfError::ResourceLimit("CMap Unicode size".into()))?;
    let mut lease = budget.reserve(capacity)?;
    let mut output = String::new();
    output
        .try_reserve_exact(capacity)
        .map_err(|_| PdfError::ResourceLimit("CMap Unicode allocation".into()))?;
    lease.resize(output.capacity())?;
    let (result, read, _) = decoder.decode_to_string(bytes, &mut output, true);
    if result != encoding_rs::CoderResult::InputEmpty || read != bytes.len() {
        return Err(PdfError::ResourceLimit("CMap Unicode output buffer".into()));
    }
    Ok((output, lease))
}

type CmapResolver<'a> = &'a dyn Fn(&str) -> PdfResult<Option<Vec<u8>>>;

pub struct CmapParser<'a> {
    data: Rc<Vec<u8>>,
    pos: usize,
    remaining_entries: usize,
    remaining_depth: usize,
    remaining_source_bytes: usize,
    inheritance_names: Vec<String>,
    resolver: Option<CmapResolver<'a>>,
    budget: Option<crate::budget::OperationBudget>,
    retained_unicode: Option<crate::budget::MemoryLease>,
    space_table: Option<crate::budget::MemoryLease>,
    unicode_table: Option<crate::budget::MemoryLease>,
    cid_table: Option<crate::budget::MemoryLease>,
}

impl<'a> CmapParser<'a> {
    pub fn new(data: Vec<u8>) -> Self {
        Self {
            data: Rc::new(data),
            pos: 0,
            remaining_entries: crate::parser::options::Limits::default().max_objects,
            remaining_depth: crate::parser::options::Limits::default().max_depth,
            remaining_source_bytes: crate::parser::options::Limits::default().max_decoded_bytes,
            inheritance_names: Vec::new(),
            resolver: None,
            budget: None,
            retained_unicode: None,
            space_table: None,
            unicode_table: None,
            cid_table: None,
        }
    }

    fn reserve_entries(&mut self, count: u64) -> PdfResult<()> {
        if count > self.remaining_entries as u64 {
            return Err(PdfError::ResourceLimit("CMap mapping entries".to_string()));
        }
        self.remaining_entries -= count as usize;
        Ok(())
    }

    pub fn with_limits(data: Vec<u8>, limits: &crate::parser::options::Limits) -> Self {
        Self {
            data: Rc::new(data),
            pos: 0,
            remaining_entries: limits.max_objects,
            remaining_depth: limits.max_depth,
            remaining_source_bytes: limits.max_decoded_bytes,
            inheritance_names: Vec::new(),
            resolver: None,
            budget: None,
            retained_unicode: None,
            space_table: None,
            unicode_table: None,
            cid_table: None,
        }
    }

    fn inherited_cmap(&mut self, name: &[u8]) -> PdfResult<Cmap> {
        if self.remaining_depth == 0 {
            return Err(PdfError::ResourceLimit("CMap inheritance depth".into()));
        }
        let name = std::str::from_utf8(name)
            .map_err(|_| PdfError::FontError("Invalid base CMap name".into()))?;
        if self
            .inheritance_names
            .iter()
            .any(|ancestor| ancestor == name)
        {
            return Err(PdfError::FontError(format!(
                "CMap inheritance cycle at {name}"
            )));
        }
        let chain_len = self
            .inheritance_names
            .len()
            .checked_add(1)
            .ok_or_else(|| PdfError::ResourceLimit("CMap inheritance chain size".into()))?;
        let mut chain_lease = self
            .budget
            .as_ref()
            .map(|pool| {
                pool.reserve(
                    chain_len
                        .checked_mul(std::mem::size_of::<String>())
                        .ok_or_else(|| {
                            PdfError::ResourceLimit("CMap inheritance chain size".into())
                        })?,
                )
            })
            .transpose()?;
        let mut inheritance_names = Vec::new();
        inheritance_names
            .try_reserve_exact(chain_len)
            .map_err(|_| PdfError::ResourceLimit("CMap inheritance chain allocation".into()))?;
        if let Some(lease) = &mut chain_lease {
            lease.resize(
                inheritance_names
                    .capacity()
                    .checked_mul(std::mem::size_of::<String>())
                    .ok_or_else(|| {
                        PdfError::ResourceLimit("CMap inheritance chain capacity".into())
                    })?,
            )?;
        }
        for ancestor in self
            .inheritance_names
            .iter()
            .map(String::as_str)
            .chain(std::iter::once(name))
        {
            let copy = if let Some(pool) = &self.budget {
                let (copy, mut lease) = crate::budget::copy_string(ancestor, pool)?;
                chain_lease.as_mut().unwrap().absorb(&mut lease)?;
                copy
            } else {
                let mut copy = String::new();
                copy.try_reserve_exact(ancestor.len()).map_err(|_| {
                    PdfError::ResourceLimit("CMap inheritance name allocation".into())
                })?;
                copy.push_str(ancestor);
                copy
            };
            inheritance_names.push(copy);
        }
        let resolved = match self.resolver {
            Some(resolve) => resolve(name)?,
            None => None,
        };
        let mut source_lease = None;
        let copy = if let Some(data) = resolved {
            if data.len() > self.remaining_source_bytes {
                return Err(PdfError::ResourceLimit(
                    "Inherited CMap source bytes".into(),
                ));
            }
            if let Some(pool) = &self.budget {
                // Resolver allocations are caller-owned; charge accepted source capacity.
                source_lease = Some(pool.reserve(data.capacity())?);
            }
            data
        } else {
            let data = font_data::cmap::get_predefine_cmap_data(name)
                .ok_or_else(|| PdfError::UnsupportedFeature(format!("Unknown base CMap {name}")))?;
            if data.len() > self.remaining_source_bytes {
                return Err(PdfError::ResourceLimit(
                    "Inherited CMap source bytes".into(),
                ));
            }
            if let Some(pool) = &self.budget {
                let (copy, lease) = crate::budget::BudgetedBytes::copy_from_slice(pool, data)?
                    .into_reserved_parts();
                source_lease = Some(lease);
                copy
            } else {
                let mut copy = Vec::new();
                copy.try_reserve_exact(data.len())
                    .map_err(|_| PdfError::ResourceLimit("Inherited CMap allocation".into()))?;
                copy.extend_from_slice(data);
                copy
            }
        };
        let mut parser = Self {
            data: Rc::new(copy),
            pos: 0,
            remaining_entries: self.remaining_entries,
            remaining_depth: self.remaining_depth - 1,
            remaining_source_bytes: self.remaining_source_bytes,
            inheritance_names,
            resolver: self.resolver,
            budget: self.budget.clone(),
            retained_unicode: None,
            space_table: None,
            unicode_table: None,
            cid_table: None,
        };
        let cmap = parser.parse()?;
        if let Some(lease) = parser.take_retained_lease() {
            self.retain_unicode_lease(lease)?;
        }
        self.remaining_entries = parser.remaining_entries;
        self.remaining_source_bytes = parser.remaining_source_bytes;
        // Keep source quota until the parser (including source views) has dropped.
        drop(parser);
        drop(source_lease);
        drop(chain_lease);
        Ok(cmap)
    }

    fn range_length(&self, low: u32, high: u32) -> PdfResult<u64> {
        if low > high {
            return Err(PdfError::FontError("Reversed CMap range".to_string()));
        }
        Ok(u64::from(high) - u64::from(low) + 1)
    }

    pub fn set_resolver(&mut self, resolver: &'a dyn Fn(&str) -> PdfResult<Option<Vec<u8>>>) {
        self.resolver = Some(resolver);
    }

    pub(crate) fn set_budget(&mut self, budget: &crate::budget::OperationBudget) {
        self.budget = Some(budget.clone());
    }

    pub(crate) fn take_retained_lease(&mut self) -> Option<crate::budget::MemoryLease> {
        for mut table in [
            self.space_table.take(),
            self.unicode_table.take(),
            self.cid_table.take(),
        ]
        .into_iter()
        .flatten()
        {
            if let Some(retained) = &mut self.retained_unicode {
                retained
                    .absorb(&mut table)
                    .expect("CMap leases share one budget and fit its live total");
            } else {
                self.retained_unicode = Some(table);
            }
        }
        self.retained_unicode.take()
    }

    fn reserve_spaces(&mut self, cmap: &mut Cmap, additional: usize) -> PdfResult<()> {
        let Some(pool) = &self.budget else {
            return Ok(());
        };
        let needed = cmap
            .code_space_range
            .len()
            .checked_add(additional)
            .ok_or_else(|| PdfError::ResourceLimit("CMap spaces size".into()))?;
        if needed <= cmap.code_space_range.capacity() {
            return Ok(());
        }
        let capacity = cmap
            .code_space_range
            .capacity()
            .max(1)
            .checked_mul(2)
            .unwrap_or(needed)
            .max(needed);
        let slots = std::mem::size_of::<CodeSpaceRange>();
        let bytes = capacity
            .checked_mul(slots)
            .ok_or_else(|| PdfError::ResourceLimit("CMap spaces allocation size".into()))?;
        let mut lease = pool.reserve(bytes)?;
        let mut next = Vec::new();
        next.try_reserve_exact(capacity)
            .map_err(|_| PdfError::ResourceLimit("CMap spaces allocation".into()))?;
        lease.resize(
            next.capacity()
                .checked_mul(slots)
                .ok_or_else(|| PdfError::ResourceLimit("CMap spaces capacity".into()))?,
        )?;
        next.append(&mut cmap.code_space_range);
        cmap.code_space_range = next;
        self.space_table = Some(lease);
        Ok(())
    }

    fn retain_unicode_lease(&mut self, mut lease: crate::budget::MemoryLease) -> PdfResult<()> {
        if let Some(retained) = &mut self.retained_unicode {
            retained.absorb(&mut lease)?;
        } else {
            self.retained_unicode = Some(lease);
        }
        Ok(())
    }

    fn add_unicode(&mut self, cmap: &mut Cmap, code: (u8, u32), bytes: &[u8]) -> PdfResult<()> {
        let text = if let Some(pool) = &self.budget {
            let (text, lease) = decode_utf16be_with_budget(bytes, pool)?;
            self.retain_unicode_lease(lease)?;
            text
        } else {
            decode_utf16be(bytes)
        };
        if let Some(pool) = &self.budget
            && cmap.cid_to_unicode.get(&code).is_none()
        {
            cmap.cid_to_unicode
                .reserve_with_budget(1, pool, &mut self.unicode_table)?;
        }
        cmap.cid_to_unicode.insert(code, text);
        Ok(())
    }

    fn add_cid(&mut self, cmap: &mut Cmap, code: (u8, u32), cid: u32) -> PdfResult<()> {
        if let Some(pool) = &self.budget
            && cmap.code_to_cids.get(&code).is_none()
        {
            cmap.code_to_cids
                .reserve_with_budget(1, pool, &mut self.cid_table)?;
        }
        cmap.code_to_cids.insert(code, cid);
        Ok(())
    }

    fn reserve_inherited_mappings(&mut self, cmap: &mut Cmap, base: &Cmap) -> PdfResult<()> {
        if let Some(pool) = &self.budget {
            cmap.cid_to_unicode.reserve_with_budget(
                base.cid_to_unicode.len(),
                pool,
                &mut self.unicode_table,
            )?;
            cmap.code_to_cids.reserve_with_budget(
                base.code_to_cids.len(),
                pool,
                &mut self.cid_table,
            )?;
        }
        Ok(())
    }

    pub fn parse_with_named_base(&mut self, name: &str) -> PdfResult<Cmap> {
        if self.data.len() > self.remaining_source_bytes {
            return Err(PdfError::ResourceLimit("CMap source bytes".into()));
        }
        let base = self.inherited_cmap(name.as_bytes())?;
        let mut local = self.parse()?;
        self.reserve_spaces(&mut local, base.code_space_range.len())?;
        self.reserve_inherited_mappings(&mut local, &base)?;
        local.inherit_defaults(base)?;
        Ok(local)
    }

    fn skip_space_and_comment(&mut self) {
        loop {
            if self.pos >= self.data.len() {
                break;
            }
            while self.pos < self.data.len() && is_white_space(self.data[self.pos]) {
                self.pos += 1;
            }
            if self.pos >= self.data.len() {
                return;
            }
            let c = self.data[self.pos];
            if c != b'%' {
                return;
            }
            loop {
                if self.pos >= self.data.len() {
                    break;
                }
                if is_end_of_line(self.data[self.pos]) {
                    break;
                }
                self.pos += 1;
            }
        }
    }
    fn word(&self, start: usize, end: usize) -> Word {
        Word {
            source: self.data.clone(),
            start,
            end,
        }
    }

    fn handle_non_delimiter(&mut self) -> Option<Word> {
        let start = self.pos;
        while self.pos < self.data.len() {
            let c = self.data[self.pos];
            if is_delimiter(c) || is_white_space(c) {
                if start == self.pos {
                    return None;
                }
                return Some(self.word(start, self.pos));
            }
            self.pos += 1;
        }
        Some(self.word(start, self.pos))
    }
    fn handle_name(&mut self) -> Option<Word> {
        if self.pos >= self.data.len() {
            return None;
        }
        self.pos += 1;
        let start = self.pos;
        while self.pos < self.data.len() {
            let c = self.data[self.pos];
            if is_delimiter(c) || is_white_space(c) {
                // Empty literal names are tokens, not EOF. Some producers emit
                // /CMapName //SubsetName before the actual mapping sections.
                return Some(self.word(start, self.pos));
            }
            self.pos += 1;
        }
        Some(self.word(start, self.pos))
    }

    fn handle_begin_angle_bracket(&mut self) -> Option<Word> {
        if self.pos > self.data.len() {
            return None;
        }
        let start = self.pos;
        while self.pos < self.data.len() {
            let c = self.data[self.pos];
            self.pos += 1;
            if c == b'>' {
                break;
            }
        }
        if start == self.pos {
            return None;
        }
        Some(self.word(start, self.pos))
    }

    fn handle_parentness(&mut self) -> Option<Word> {
        if self.pos > self.data.len() {
            return None;
        }
        self.pos += 1;
        let start = self.pos;
        let mut level = 1;
        while self.pos < self.data.len() && level > 0 {
            let c = self.data[self.pos];
            if c == b'(' {
                level += 1;
            } else if c == b')' {
                level -= 1;
            }
            self.pos += 1;
        }
        if start == self.pos {
            return None;
        }
        Some(self.word(start, self.pos))
    }

    fn handle_dict(&mut self) -> Option<Word> {
        self.pos += 2;
        let start = self.pos;
        if self.pos >= self.data.len() {
            return None;
        }
        loop {
            if self.pos + 1 >= self.data.len() {
                self.pos = self.data.len();
                return None;
            }
            let c = self.data[self.pos];
            if c == b'>' && self.data[self.pos + 1] == b'>' {
                let end = self.pos;
                self.pos += 2;
                return Some(self.word(start, end));
            }
            self.pos += 1;
        }
    }

    fn handle_array(&mut self) -> Option<Word> {
        let start = self.pos;
        self.pos += 1;
        if self.pos >= self.data.len() {
            return None;
        }
        loop {
            if self.pos >= self.data.len() {
                return None;
            }
            let c = self.data[self.pos];
            if c == b']' {
                self.pos += 1;
                break;
            }
            self.pos += 1;
        }
        Some(self.word(start, self.pos))
    }

    fn get_next_word(&mut self) -> Option<Word> {
        self.skip_space_and_comment();
        if self.pos >= self.data.len() {
            return None;
        }
        let c = self.data[self.pos];
        if !is_delimiter(c) {
            return self.handle_non_delimiter();
        }
        match c {
            b'/' => self.handle_name(),
            b'<' => {
                if self.pos + 1 < self.data.len() && self.data[self.pos + 1] == b'<' {
                    self.handle_dict()
                } else {
                    self.handle_begin_angle_bracket()
                }
            }
            b'[' => self.handle_array(),
            b'(' => self.handle_parentness(),
            _ => None,
        }
    }
    fn required_word(&mut self) -> PdfResult<Word> {
        self.get_next_word().ok_or_else(|| {
            PdfError::FontError(format!("Truncated CMap entry at byte {}", self.pos))
        })
    }
    fn get_code(&self, bytes: &[u8]) -> PdfResult<u32> {
        if bytes.first() == Some(&b'<') {
            let decoded = self.hex_to_bytes(bytes)?;
            if decoded.is_empty() || decoded.len() > 4 {
                return Err(PdfError::FontError(
                    "CMap code must contain 1–4 bytes".to_string(),
                ));
            }
            return Ok(decoded
                .iter()
                .fold(0u32, |value, byte| (value << 8) | u32::from(*byte)));
        }
        std::str::from_utf8(bytes)
            .ok()
            .and_then(|s| s.parse().ok())
            .ok_or_else(|| PdfError::FontError("Invalid CMap integer".to_string()))
    }

    fn process_codespace_range(&mut self, cmap: &mut Cmap, n: usize) -> PdfResult<()> {
        for _ in 0..n {
            let low = self.required_word()?;
            let high = self.required_word()?;
            let l = self.get_code(&low)?;
            let h = self.get_code(&high)?;
            self.range_length(l, h)?;
            let size = self.hex_to_bytes(&high)?.len();
            if self.hex_to_bytes(&low)?.len() != size {
                return Err(PdfError::FontError(
                    "CMap code space widths differ".to_string(),
                ));
            }
            self.reserve_entries(1)?;
            self.reserve_spaces(cmap, 1)?;
            cmap.add_code_space_range(CodeSpaceRange::new(l, h, size as u8));
        }
        Ok(())
    }

    fn process_cid_range(&mut self, cmap: &mut Cmap, n: usize) -> PdfResult<()> {
        for _ in 0..n {
            let start = self.required_word()?;
            let end = self.required_word()?;
            let val = self.required_word()?;
            let l = self.get_code(start.as_slice())?;
            let h = self.get_code(end.as_slice())?;
            let length = self.source_length(&start)?;
            if length != self.source_length(&end)? {
                return Err(PdfError::FontError(
                    "CMap range source lengths differ".into(),
                ));
            }
            let gid = self.get_code(&val)?;
            let count = self.range_length(l, h)?;
            if u64::from(gid) + count - 1 > u64::from(u32::MAX) {
                return Err(PdfError::FontError("CMap CID range overflows".to_string()));
            }
            self.reserve_entries(count)?;
            for code in l..=h {
                self.add_cid(cmap, (length, code), gid + (code - l))?;
            }
        }
        Ok(())
    }

    fn process_cid_char(&mut self, cmap: &mut Cmap, n: usize) -> PdfResult<()> {
        for _ in 0..n {
            let key = self.required_word()?;
            let val = self.required_word()?;
            let cid = self.get_code(&key)?;
            let gid = self.get_code(&val)?;
            self.reserve_entries(1)?;
            self.add_cid(cmap, (self.source_length(&key)?, cid), gid)?;
        }
        Ok(())
    }

    fn process_bf_range(&mut self, cmap: &mut Cmap, n: usize) -> PdfResult<()> {
        for _ in 0..n {
            let low = self.required_word()?;
            let high = self.required_word()?;
            let l = self.get_code(&low)?;
            let h = self.get_code(&high)?;
            let length = self.source_length(&low)?;
            if length != self.source_length(&high)? {
                return Err(PdfError::FontError(
                    "CMap range source lengths differ".into(),
                ));
            }
            let val = self.required_word()?;
            let count = self.range_length(l, h)?;
            self.reserve_entries(count)?;
            if val.first() == Some(&b'[') {
                let data = val.get(1..val.len() - 1).ok_or_else(|| {
                    PdfError::FontError("Invalid CMap destination array".to_string())
                })?;
                let (data, _source_lease) = if let Some(pool) = &self.budget {
                    let (data, lease) = crate::budget::BudgetedBytes::copy_from_slice(pool, data)?
                        .into_reserved_parts();
                    (data, Some(lease))
                } else {
                    (data.to_vec(), None)
                };
                let mut destinations = CmapParser::new(data);
                destinations.budget = self.budget.clone();
                for code in l..=h {
                    let word = destinations.required_word()?;
                    let bytes = self.hex_to_bytes(&word)?;
                    self.add_unicode(cmap, (length, code), &bytes)?;
                }
                if destinations.get_next_word().is_some() {
                    return Err(PdfError::FontError(
                        "CMap destination array has extra entries".to_string(),
                    ));
                }
            } else {
                let mut unicode = self.hex_to_bytes(&val)?;
                if unicode.is_empty() || unicode.len() % 2 != 0 {
                    return Err(PdfError::FontError(
                        "CMap destination requires UTF-16BE bytes".to_string(),
                    ));
                }
                for code in l..=h {
                    self.add_unicode(cmap, (length, code), &unicode)?;
                    if code != h {
                        let mut carry = true;
                        for byte in unicode.iter_mut().rev() {
                            let (value, overflow) = byte.overflowing_add(1);
                            *byte = value;
                            carry = overflow;
                            if !carry {
                                break;
                            }
                        }
                        if carry {
                            return Err(PdfError::FontError(
                                "CMap destination range overflows".to_string(),
                            ));
                        }
                    }
                }
            }
        }
        Ok(())
    }
    fn hex_to_bytes(&self, hex: &[u8]) -> PdfResult<HexBytes> {
        let digits = hex
            .strip_prefix(b"<")
            .and_then(|s| s.strip_suffix(b">"))
            .ok_or_else(|| {
                PdfError::FontError("CMap destination must be a hexadecimal string".to_string())
            })?;
        let mut count = 0usize;
        for &digit in digits {
            if is_white_space(digit) {
                continue;
            }
            if !digit.is_ascii_hexdigit() {
                return Err(PdfError::FontError(
                    "Invalid CMap hexadecimal digit".to_string(),
                ));
            }
            count += 1;
        }
        let size = count / 2 + count % 2;
        let mut lease = self
            .budget
            .as_ref()
            .map(|pool| pool.reserve(size))
            .transpose()?;
        let mut values = Vec::new();
        values
            .try_reserve_exact(size)
            .map_err(|_| PdfError::ResourceLimit("CMap hexadecimal allocation".into()))?;
        if let Some(lease) = &mut lease {
            lease.resize(values.capacity())?;
        }
        let mut digits = digits.iter().copied().filter(|c| !is_white_space(*c));
        while let Some(high) = digits.next() {
            let low = digits.next().map(hex_to_u8).unwrap_or(0);
            values.push(hex_to_u8(high) << 4 | low);
        }
        Ok(HexBytes {
            data: values,
            _lease: lease,
        })
    }

    fn process_bf_char(&mut self, cmap: &mut Cmap, n: usize) -> PdfResult<()> {
        for _ in 0..n {
            let code = self.required_word()?;
            let code_value = self.get_code(&code)?;
            let c = self.required_word()?;
            let unicode_bytes = self.hex_to_bytes(&c)?;

            self.reserve_entries(1)?;
            self.add_unicode(
                cmap,
                (self.source_length(&code)?, code_value),
                &unicode_bytes,
            )?;
        }
        Ok(())
    }

    fn source_length(&self, token: &[u8]) -> PdfResult<u8> {
        let bytes = self.hex_to_bytes(token)?;
        if !(1..=4).contains(&bytes.len()) {
            return Err(PdfError::FontError(
                "CMap source must contain 1..4 bytes".into(),
            ));
        }
        Ok(bytes.len() as u8)
    }

    pub fn parse(&mut self) -> PdfResult<Cmap> {
        self.remaining_source_bytes = self
            .remaining_source_bytes
            .checked_sub(self.data.len())
            .ok_or_else(|| PdfError::ResourceLimit("CMap cumulative source bytes".into()))?;
        //println!("{}", String::from_utf8(self.data.clone()).unwrap());
        let mut cmap = Cmap::default();
        if self.budget.is_some() {
            cmap.cid_to_unicode.sorted = Some(Vec::new());
            cmap.code_to_cids.sorted = Some(Vec::new());
        }
        let mut last_word = self.word(0, 0);
        while self.pos < self.data.len() {
            if let Some(word) = self.get_next_word() {
                if last_word.as_slice() == b"WMode" {
                    cmap.wmode = Some(match word.as_slice() {
                        b"0" => 0,
                        b"1" => 1,
                        _ => return Err(PdfError::FontError("CMap WMode must be 0 or 1".into())),
                    });
                }
                match word.as_slice() {
                    b"endcmap" => {
                        break;
                    }
                    b"usecmap" => {
                        let base = self.inherited_cmap(&last_word)?;
                        self.reserve_spaces(&mut cmap, base.code_space_range.len())?;
                        self.reserve_inherited_mappings(&mut cmap, &base)?;
                        cmap.inherit_defaults(base)?;
                    }
                    b"begincodespacerange" => {
                        let n = self.get_code(last_word.as_slice())? as usize;
                        if n > self.remaining_entries {
                            return Err(PdfError::ResourceLimit(
                                "CMap declared entries".to_string(),
                            ));
                        }
                        self.process_codespace_range(&mut cmap, n)?;
                    }
                    b"beginbfchar" => {
                        let n = self.get_code(last_word.as_slice())? as usize;
                        if n > self.remaining_entries {
                            return Err(PdfError::ResourceLimit(
                                "CMap declared entries".to_string(),
                            ));
                        }
                        self.process_bf_char(&mut cmap, n)?;
                    }
                    b"beginbfrange" => {
                        let n = self.get_code(last_word.as_slice())? as usize;
                        if n > self.remaining_entries {
                            return Err(PdfError::ResourceLimit(
                                "CMap declared entries".to_string(),
                            ));
                        }
                        self.process_bf_range(&mut cmap, n)?;
                    }
                    b"begincidchar" => {
                        let n = self.get_code(last_word.as_slice())? as usize;
                        if n > self.remaining_entries {
                            return Err(PdfError::ResourceLimit(
                                "CMap declared entries".to_string(),
                            ));
                        }
                        self.process_cid_char(&mut cmap, n)?;
                    }
                    b"begincidrange" => {
                        let n = self.get_code(last_word.as_slice())? as usize;
                        if n > self.remaining_entries {
                            return Err(PdfError::ResourceLimit(
                                "CMap declared entries".to_string(),
                            ));
                        }
                        self.process_cid_range(&mut cmap, n)?;
                    }
                    _ => {}
                }
                last_word = word;
            } else {
                break;
            }
        }
        if last_word.as_slice() == b"WMode" {
            return Err(PdfError::FontError("CMap WMode value missing".into()));
        }
        Ok(cmap)
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn empty_literal_name_does_not_truncate_unicode_sections() {
        use crate::{
            budget::OperationBudget,
            font::{CharCode, cmap::Cmap},
        };
        let source = b"/CIDInit /ProcSet findresource begin 12 dict begin begincmap\n/CIDSystemInfo << /Registry (Subset) /Ordering (Subset) /Supplement 0 >> def\n/CMapName //Subset def\n/CMapType 2 def\n1 begincodespacerange <20> <7F> endcodespacerange\n2 beginbfchar <20> <4E2D> <21> <6587> endbfchar\nendcmap";
        let map = Cmap::try_new(source.to_vec()).unwrap();
        assert_eq!(map.unicode(&CharCode::new(32, 1, 0.)), Some("中"));
        assert_eq!(map.unicode(&CharCode::new(33, 1, 0.)), Some("文"));
        let budget = OperationBudget::new(65536);
        let mut parser = super::CmapParser::new(source.to_vec());
        parser.set_budget(&budget);
        let map = parser.parse().unwrap();
        let lease = parser.take_retained_lease();
        assert_eq!(map.unicode(&CharCode::new(32, 1, 0.)), Some("中"));
        drop((map, lease));
        drop(parser);
        assert_eq!(budget.live_bytes(), 0);
    }

    #[test]
    fn budget_nested_sources_coexist_and_release_on_limit_or_callback_error() {
        use crate::budget::OperationBudget;
        let resolver = |name: &str| -> crate::error::PdfResult<Option<Vec<u8>>> {
            match name {
                "BudgetOuter" => {
                    let mut source = vec![b' '; 1024];
                    let header = b"/BudgetInner usecmap\n%";
                    source[..header.len()].copy_from_slice(header);
                    Ok(Some(source))
                }
                "BudgetInner" => Ok(Some(vec![b'%'; 2048])),
                "Cycle" => Ok(Some(b"/Cycle usecmap".to_vec())),
                _ => Err(crate::error::PdfError::FontError("resolver failure".into())),
            }
        };
        let run = |source: &[u8], pool: &OperationBudget| {
            let mut parser = super::CmapParser::new(source.to_vec());
            parser.set_budget(pool);
            parser.set_resolver(&resolver);
            parser.parse()
        };
        let chain_bytes =
            3 * std::mem::size_of::<String>() + 2 * "BudgetOuter".len() + "BudgetInner".len();
        let pool = OperationBudget::new(3072 + chain_bytes);
        assert!(run(b"/BudgetOuter usecmap", &pool).is_ok());
        assert_eq!(pool.peak_bytes(), 3072 + chain_bytes);
        assert_eq!(pool.live_bytes(), 0);
        let neighbor = pool.reserve(1).unwrap();
        assert!(matches!(
            run(b"/BudgetOuter usecmap", &pool),
            Err(crate::error::PdfError::ResourceLimit(_))
        ));
        assert_eq!(pool.live_bytes(), 1);
        drop(neighbor);
        assert!(run(b"/BudgetOuter usecmap", &pool).is_ok());
        assert_eq!(pool.live_bytes(), 0);
        assert!(run(b"/Unknown usecmap", &pool).is_err());
        assert_eq!(pool.live_bytes(), 0);
        assert!(
            matches!(run(b"/Cycle usecmap", &pool), Err(crate::error::PdfError::FontError(message)) if message.contains("cycle"))
        );
        assert_eq!(pool.live_bytes(), 0);
    }

    #[test]
    fn budget_code_spaces_count_capacity_growth_and_inherited_overlap() {
        use crate::budget::{MemoryLease, OperationBudget};
        let resolver = |name: &str| -> crate::error::PdfResult<Option<Vec<u8>>> {
            Ok((name == "BudgetSpaces")
                .then(|| b"1 begincodespacerange <0000> <FFFF> endcodespacerange".to_vec()))
        };
        let run = |source: &str,
                   pool: &OperationBudget|
         -> crate::error::PdfResult<(super::Cmap, MemoryLease)> {
            let mut parser = super::CmapParser::new(source.as_bytes().to_vec());
            parser.set_budget(pool);
            parser.set_resolver(&resolver);
            let cmap = parser.parse()?;
            let lease = parser.take_retained_lease().unwrap();
            Ok((cmap, lease))
        };
        let slots = std::mem::size_of::<super::CodeSpaceRange>();
        let local = "3 begincodespacerange <00> <7F> <80> <FF> <0000> <FFFF> endcodespacerange";
        let probe = OperationBudget::new(65536);
        let (cmap, lease) = run(local, &probe).unwrap();
        assert_eq!(cmap.code_space_range.len(), 3);
        assert_eq!(lease.bytes(), cmap.code_space_range.capacity() * slots);
        assert_eq!(probe.peak_bytes(), 6 * slots);
        assert_eq!(probe.live_bytes(), lease.bytes());
        drop((cmap, lease));
        assert_eq!(probe.live_bytes(), 0);
        let local = "2 begincodespacerange <00> <7F> <80> <FF> endcodespacerange";
        for source in [
            format!("/BudgetSpaces usecmap {local}"),
            format!("{local} /BudgetSpaces usecmap"),
        ] {
            let probe = OperationBudget::new(65536);
            let (cmap, lease) = run(&source, &probe).unwrap();
            assert_eq!(cmap.code_space_range.len(), 3);
            assert_eq!(lease.bytes(), 6 * slots); // Retains the consumed base table conservatively.
            assert_eq!(probe.live_bytes(), lease.bytes());
            assert!(probe.peak_bytes() >= 8 * slots);
            let peak = probe.peak_bytes();
            drop((cmap, lease));
            assert_eq!(probe.live_bytes(), 0);
            let pool = OperationBudget::new(peak);
            let neighbor = pool.reserve(1).unwrap();
            assert!(matches!(
                run(&source, &pool),
                Err(crate::error::PdfError::ResourceLimit(_))
            ));
            assert_eq!(pool.live_bytes(), 1);
            drop(neighbor);
            drop(run(&source, &pool).unwrap());
            assert_eq!(pool.live_bytes(), 0);
            assert!(run("/BudgetSpaces usecmap 1 begincodespacerange <00>", &pool).is_err());
            assert_eq!(pool.live_bytes(), 0);
        }
    }

    #[test]
    fn budget_unicode_ranges_and_inheritance_retain_quota_and_recover() {
        use crate::budget::{MemoryLease, OperationBudget};
        let resolver = |name: &str| -> crate::error::PdfResult<Option<Vec<u8>>> {
            Ok((name == "BudgetBase")
                .then(|| b"2 beginbfchar <01> <0041> <02> <0042> endbfchar".to_vec()))
        };
        let local = "1 beginbfchar <01> <0043> endbfchar 1 beginbfrange <03> <04> [<00410042><D83DDE00>] endbfrange 1 beginbfrange <05> <06> <0044> endbfrange";
        let cases = [
            (format!("/BudgetBase usecmap {local}"), false),
            (format!("{local} /BudgetBase usecmap"), false),
            (local.to_string(), true),
        ];
        let run = |source: &str,
                   named: bool,
                   pool: &OperationBudget|
         -> crate::error::PdfResult<(super::Cmap, MemoryLease)> {
            let mut parser = super::CmapParser::new(source.as_bytes().to_vec());
            parser.set_resolver(&resolver);
            parser.set_budget(pool);
            let cmap = if named {
                parser.parse_with_named_base("BudgetBase")?
            } else {
                parser.parse()?
            };
            let lease = parser.take_retained_lease().unwrap();
            Ok((cmap, lease))
        };
        let expected_pool = OperationBudget::new(65536);
        let expected_retained: usize = [
            &b"\0A"[..],
            &b"\0B"[..],
            &b"\0C"[..],
            &b"\0A\0B"[..],
            &b"\xd8\x3d\xde\0"[..],
            &b"\0D"[..],
            &b"\0E"[..],
        ]
        .into_iter()
        .map(|bytes| {
            let (text, lease) = super::decode_utf16be_with_budget(bytes, &expected_pool).unwrap();
            let capacity = text.capacity();
            drop((text, lease));
            capacity
        })
        .sum();
        for (source, named) in cases {
            let probe = OperationBudget::new(65536);
            let (cmap, lease) = run(&source, named, &probe).unwrap();
            for (code, text) in [(1, "C"), (2, "B"), (3, "AB"), (4, "😀"), (5, "D"), (6, "E")] {
                assert_eq!(
                    cmap.cid_to_unicode.get(&(1, code)).map(String::as_str),
                    Some(text)
                );
            }
            assert_eq!(
                lease.bytes(),
                expected_retained + 10 * std::mem::size_of::<((u8, u32), String)>()
            );
            assert_eq!(probe.live_bytes(), lease.bytes());
            let peak = probe.peak_bytes();
            drop((cmap, lease));
            assert_eq!(probe.live_bytes(), 0);
            let pool = OperationBudget::new(peak);
            let neighbor = pool.reserve(1).unwrap();
            assert!(matches!(
                run(&source, named, &pool),
                Err(crate::error::PdfError::ResourceLimit(_))
            ));
            assert_eq!(pool.live_bytes(), 1);
            drop(neighbor);
            drop(run(&source, named, &pool).unwrap());
            assert_eq!(pool.live_bytes(), 0);
            let malformed = "/BudgetBase usecmap 1 beginbfrange <03> <04> [<0041>] endbfrange";
            assert!(run(malformed, false, &pool).is_err());
            assert_eq!(pool.live_bytes(), 0);
        }
    }

    #[test]
    fn budget_unicode_decoder_matches_boms_malformed_sequences_and_capacity() {
        let pool = crate::budget::OperationBudget::new(65536);
        for bytes in [
            &b""[..],
            &b"\0A\0B"[..],
            &b"\xd8\x3d\xde\0"[..],
            &b"\xd8\0\0A"[..],
            &b"\xd8\0\xff"[..],
            &b"\xff\xfeA\0"[..],
            &b"\xfe\xff\0A"[..],
            &b"\xef\xbb\xbfA"[..],
        ] {
            let (text, lease) = super::decode_utf16be_with_budget(bytes, &pool).unwrap();
            assert_eq!(text, super::decode_utf16be(bytes));
            assert_eq!(lease.bytes(), text.capacity());
            assert_eq!(pool.live_bytes(), lease.bytes());
            drop((text, lease));
            assert_eq!(pool.live_bytes(), 0);
        }
        for unit in 0..=u16::MAX {
            let bytes = unit.to_be_bytes();
            let result = super::decode_utf16be_with_budget(&bytes, &pool).unwrap();
            assert_eq!(result.0, super::decode_utf16be(&bytes), "unit {unit}");
            drop(result);
        }
        assert_eq!(pool.live_bytes(), 0);
        let bytes = b"\0A";
        let (_, lease) = super::decode_utf16be_with_budget(bytes, &pool).unwrap();
        let capacity = lease.bytes();
        drop(lease);
        let exact = crate::budget::OperationBudget::new(capacity);
        let neighbor = exact.reserve(1).unwrap();
        assert!(super::decode_utf16be_with_budget(bytes, &exact).is_err());
        assert_eq!(exact.live_bytes(), 1);
        drop(neighbor);
        drop(super::decode_utf16be_with_budget(bytes, &exact).unwrap());
        assert_eq!(exact.live_bytes(), 0);
    }

    #[test]
    fn hexadecimal_budget_tracks_live_capacity_overlap_and_recovery() {
        let pool = crate::budget::OperationBudget::new(4);
        let mut parser = super::CmapParser::new(Vec::new());
        parser.set_budget(&pool);
        let first = parser.hex_to_bytes(b"<0041>").unwrap();
        assert_eq!(first.as_slice(), &[0, 65]);
        assert_eq!(pool.live_bytes(), first.capacity());
        let second = parser.hex_to_bytes(b"<0042>").unwrap();
        assert_eq!(pool.live_bytes(), first.capacity() + second.capacity());
        assert_eq!(pool.peak_bytes(), 4);
        assert!(parser.hex_to_bytes(b"<01>").is_err());
        assert_eq!(pool.live_bytes(), 4);
        drop(second);
        let neighbor = pool.reserve(1).unwrap();
        assert!(parser.hex_to_bytes(b"<0042>").is_err());
        assert_eq!(pool.live_bytes(), 3);
        assert!(parser.hex_to_bytes(b"<004g>").is_err());
        assert_eq!(pool.live_bytes(), 3);
        drop(neighbor);
        let result = parser.hex_to_bytes(b"<0042>").unwrap();
        drop((first, result));
        assert_eq!(pool.live_bytes(), 0);
        let pool = crate::budget::OperationBudget::new(65536);
        let mut parser = super::CmapParser::new(b"1 beginbfchar <41> <0041> endbfchar".to_vec());
        parser.set_budget(&pool);
        let cmap = parser.parse().unwrap();
        assert_eq!(
            cmap.unicode(&crate::font::CharCode::new(65, 1, 0.)),
            Some("A")
        );
        let lease = parser.take_retained_lease().unwrap();
        assert_eq!(pool.live_bytes(), lease.bytes());
        assert!(lease.bytes() > 0);
        drop((cmap, lease));
        assert_eq!(pool.live_bytes(), 0);
    }

    #[test]
    fn token_views_share_source_and_preserve_token_boundaries() {
        let mut parser = super::CmapParser::new(
            b"% comment\n /Name plain <00ff> [<0041><0042>] (a(b)c) << /Key 1 >>".to_vec(),
        );
        let source = parser.data.clone();
        for expected in [
            &b"Name"[..],
            &b"plain"[..],
            &b"<00ff>"[..],
            &b"[<0041><0042>]"[..],
            &b"a(b)c)"[..],
            &b" /Key 1 "[..],
        ] {
            let word = parser.required_word().unwrap();
            assert_eq!(word.as_slice(), expected);
            assert!(std::rc::Rc::ptr_eq(&word.source, &source));
            assert_eq!(word.as_slice().as_ptr(), source[word.start..].as_ptr());
        }
        assert!(parser.get_next_word().is_none());
        let mut parser = super::CmapParser::new(b"/Final".to_vec());
        let word = parser.required_word().unwrap();
        drop(parser);
        assert_eq!(word.as_slice(), b"Final");
        assert_eq!(
            super::CmapParser::new(b"/".to_vec())
                .required_word()
                .unwrap()
                .as_slice(),
            b""
        );
        for truncated in [&b"[<41>"[..], &b"<< /Key"[..]] {
            assert!(
                super::CmapParser::new(truncated.to_vec())
                    .get_next_word()
                    .is_none()
            );
        }
    }

    #[test]
    fn hexadecimal_decoding_preserves_whitespace_odd_nibbles_and_validation() {
        let parser = super::CmapParser::new(Vec::new());
        for (input, expected) in [
            (&b"<>"[..], &b""[..]),
            (&b"<4 1\t4\n2\r4\x0c3\0>"[..], &b"ABC"[..]),
            (&b"<aB c>"[..], &b"\xab\xc0"[..]),
            (&b"<0>"[..], &b"\0"[..]),
        ] {
            assert_eq!(parser.hex_to_bytes(input).unwrap().as_slice(), expected);
        }
        for input in [&b"<41g>"[..], &b"<41"[..], &b"41>"[..], &b"<4\x014>"[..]] {
            assert!(parser.hex_to_bytes(input).is_err());
        }
        assert_eq!(super::decode_utf16be(&[0, 65, 0, 66]), "AB");
        assert_eq!(super::decode_utf16be(&[0xd8, 0x3d, 0xde, 0]), "😀");
        assert_eq!(super::decode_utf16be(&[0xd8, 0, 0, 65]), "�A");
    }

    #[test]
    fn embedded_usecmap_is_not_silently_ignored() {
        assert!(matches!(
            super::CmapParser::new(b"/Unknown-Rzpdf-CMap usecmap".to_vec()).parse(),
            Err(crate::error::PdfError::UnsupportedFeature(_))
        ));
    }
    use crate::font::cmap::parser::CmapParser;

    #[test]
    fn writing_mode_values_are_checked_and_local_mode_wins() {
        for data in [
            b"/WMode 2 def".as_slice(),
            b"/WMode -1 def",
            b"/WMode 1.5 def",
            b"/WMode /Bad def",
            b"/WMode",
        ] {
            assert!(CmapParser::new(data.to_vec()).parse().is_err());
        }
        for data in [
            b"/WMode 0 def /Identity-V usecmap".as_slice(),
            b"/Identity-V usecmap /WMode 0 def",
        ] {
            assert_eq!(
                CmapParser::new(data.to_vec()).parse().unwrap().wmode(),
                Some(0)
            );
        }
    }

    #[test]
    fn inheritance_cycle_detection_is_path_local() {
        let mut parser = CmapParser::new(b"/Identity-H usecmap".to_vec());
        parser.inheritance_names.push("Identity-H".into());
        assert!(
            matches!(parser.parse(), Err(crate::error::PdfError::FontError(message)) if message.contains("cycle"))
        );
        let cmap = CmapParser::new(b"/Identity-H usecmap /Identity-H usecmap".to_vec())
            .parse()
            .unwrap();
        assert_eq!(cmap.cid_for_char(&crate::font::CharCode::new(1, 2, 0.)), 1);
    }

    #[test]
    fn inherited_identity_map_has_local_overrides_and_shared_limits() {
        let inherited = b"/Identity-H usecmap".to_vec();
        let size = inherited.len()
            + font_data::cmap::get_predefine_cmap_data("Identity-H")
                .unwrap()
                .len();
        for (budget, success) in [(size, true), (size - 1, false)] {
            let limits = crate::parser::options::Limits {
                max_decoded_bytes: budget,
                ..Default::default()
            };
            assert_eq!(
                CmapParser::with_limits(inherited.clone(), &limits)
                    .parse()
                    .is_ok(),
                success
            );
        }
        let data = b"1 begincidchar <0001> 42 endcidchar /Identity-H usecmap /WMode 1 def".to_vec();
        let cmap = CmapParser::new(data.clone()).parse().unwrap();
        assert_eq!(cmap.cid_for_char(&crate::font::CharCode::new(1, 2, 0.)), 42);
        assert_eq!(cmap.cid_for_char(&crate::font::CharCode::new(2, 2, 0.)), 2);
        assert_eq!(cmap.wmode(), Some(1));
        assert_eq!(cmap.chars_limited(&[0, 1], 1).unwrap().len(), 1);
        let after =
            CmapParser::new(b"/Identity-H usecmap 1 begincidchar <0001> 42 endcidchar".to_vec())
                .parse()
                .unwrap();
        assert_eq!(
            after.cid_for_char(&crate::font::CharCode::new(1, 2, 0.)),
            42
        );
        assert_eq!(after.cid_for_char(&crate::font::CharCode::new(2, 2, 0.)), 2);
        let limits = crate::parser::options::Limits {
            max_depth: 0,
            ..Default::default()
        };
        assert!(
            CmapParser::with_limits(data.clone(), &limits)
                .parse()
                .is_err()
        );
        let limits = crate::parser::options::Limits {
            max_objects: 65537,
            ..Default::default()
        };
        assert!(CmapParser::with_limits(data, &limits).parse().is_err());
    }

    #[test]
    fn inclusive_ranges_and_utf16_destinations() {
        let cmap = CmapParser::new(
            b"1 begincidrange <01> <02> 10 1 beginbfrange <01> <02> <D83DDE00>".to_vec(),
        )
        .parse()
        .unwrap();
        assert_eq!(cmap.cid_for_char(&crate::font::CharCode::new(1, 1, 0.)), 10);
        assert_eq!(cmap.cid_for_char(&crate::font::CharCode::new(2, 1, 0.)), 11);
        assert_eq!(
            cmap.unicode(&crate::font::CharCode::new(1, 1, 0.)),
            Some("😀")
        );
        assert_eq!(
            cmap.unicode(&crate::font::CharCode::new(2, 1, 0.)),
            Some("😁")
        );
        let cmap = CmapParser::new(b"1 beginbfrange <01> <02> [<0041><00420043>]".to_vec())
            .parse()
            .unwrap();
        assert_eq!(
            cmap.unicode(&crate::font::CharCode::new(2, 1, 0.)),
            Some("BC")
        );
    }

    #[test]
    fn invalid_ranges_and_mapping_budget_return_errors() {
        for input in [
            b"1 begincidrange <02> <01> 0".as_slice(),
            b"1 begincidrange <00> <01> 4294967295",
            b"1 begincidrange <00000000> <FFFFFFFF> 0",
            b"-1 beginbfchar",
            b"1 beginbfrange <01> <02> <FFFF>",
        ] {
            assert!(CmapParser::new(input.to_vec()).parse().is_err());
        }
        let mut parser = CmapParser::new(b"1 begincidrange <01> <02> 0".to_vec());
        parser.remaining_entries = 1;
        assert!(parser.parse().is_err());
        let mut parser = CmapParser::new(b"1 begincidrange <01> <02> 0".to_vec());
        parser.remaining_entries = 2;
        assert!(parser.parse().is_ok());
    }

    #[test]
    fn malformed_and_truncated_cmap_inputs_do_not_panic() {
        for input in [
            b"1 beginbfchar <01>".as_slice(),
            b"1 beginbfchar <01> <GG>",
            b"1 beginbfchar <0102030405> <0041>",
            b"1 beginbfchar <01> [",
        ] {
            assert!(CmapParser::new(input.to_vec()).parse().is_err());
        }
        for input in [b"   ".as_slice(), b"% trailing comment", b"<<", b"["] {
            assert!(CmapParser::new(input.to_vec()).parse().is_ok());
        }
        assert!(
            CmapParser::new(b"1 beginbfchar <01><0041> endbfchar".to_vec())
                .parse()
                .is_ok()
        );
    }

    #[test]
    fn test_cmap_parse() {
        let content = b"
%! commant
%% commant
/CIDInit /ProcSet findresource begin 12 dict begin begincmap /CIDSystemInfo <<
/Registry (AAAAAA+F4+0) /Ordering (T1UV) /Supplement 0 >> def
/CMapName /AAAAAA+F4+0 def
/CMapType 2 def
1 begincodespacerange <18> <fc> endcodespacerange
15 beginbfchar
<18> <02D8>
<19> <02C7>
<21> <0021>
<5d> <005D>
<5f> <005F>
<84> <2014>
<85> <2013>
<b8> <00B8>
<e4> <00E4>
<e9> <00E9>
<ed> <00ED>
<ef> <00EF>
<f4> <00F4>
<f6> <00F6>
<fc> <00FC>
endbfchar
9 beginbfrange
<23> <26> <0023>
<28> <3b> <0028>
<3d> <3f> <003D>
<41> <5b> <0041>
<61> <7e> <0061>
<8d> <8e> <201C>
<8f> <90> <2018>
<93> <94> <FB01>
<e0> <e1> <00E0>
endbfrange
endcmap CMapName currentdict /CMap defineresource pop end end";
        let mut cmap_parser = CmapParser::new(content.to_vec());
        let cmap = cmap_parser.parse().unwrap();
        println!("{:?}", cmap.cid_to_unicode);
    }

    #[test]
    fn test_parse_cid() {
        let content = b"%!PS-Adobe-3.0 Resource-CMap
%%DocumentNeededResources: ProcSet (CIDInit)
%%IncludeResource: ProcSet (CIDInit)
%%BeginResource: CMap (83pv-RKSJ-H)
%%Title: (83pv-RKSJ-H Adobe Japan1 1)
%%Version: 10.005
%%Copyright: -----------------------------------------------------------
%%Copyright: Copyright 1990-2015 Adobe Systems Incorporated.
%%Copyright: All rights reserved.
%%Copyright:
%%Copyright: Redistribution and use in source and binary forms, with or
%%Copyright: without modification, are permitted provided that the
%%Copyright: following conditions are met:
%%Copyright:
%%Copyright: Redistributions of source code must retain the above
%%Copyright: copyright notice, this list of conditions and the following
%%Copyright: disclaimer.
%%Copyright:
%%Copyright: Redistributions in binary form must reproduce the above
%%Copyright: copyright notice, this list of conditions and the following
%%Copyright: disclaimer in the documentation and/or other materials
%%Copyright: provided with the distribution.
%%Copyright:
%%Copyright: Neither the name of Adobe Systems Incorporated nor the names
%%Copyright: of its contributors may be used to endorse or promote
%%Copyright: products derived from this software without specific prior
%%Copyright: written permission.
%%Copyright:
%%Copyright: THIS SOFTWARE IS PROVIDED BY THE COPYRIGHT HOLDERS AND
%%Copyright: INCLUDING, BUT NOT LIMITED TO, THE IMPLIED WARRANTIES OF
%%Copyright: MERCHANTABILITY AND FITNESS FOR A PARTICULAR PURPOSE ARE
%%Copyright: DISCLAIMED. IN NO EVENT SHALL THE COPYRIGHT HOLDER OR
%%Copyright: CONTRIBUTORS BE LIABLE FOR ANY DIRECT, INDIRECT, INCIDENTAL,
%%Copyright: SPECIAL, EXEMPLARY, OR CONSEQUENTIAL DAMAGES (INCLUDING, BUT
%%Copyright: NOT LIMITED TO, PROCUREMENT OF SUBSTITUTE GOODS OR SERVICES;
%%Copyright: LOSS OF USE, DATA, OR PROFITS; OR BUSINESS INTERRUPTION)
%%Copyright: HOWEVER CAUSED AND ON ANY THEORY OF LIABILITY, WHETHER IN
%%Copyright: CONTRACT, STRICT LIABILITY, OR TORT (INCLUDING NEGLIGENCE OR
%%Copyright: OTHERWISE) ARISING IN ANY WAY OUT OF THE USE OF THIS
%%Copyright: SOFTWARE, EVEN IF ADVISED OF THE POSSIBILITY OF SUCH DAMAGE.
%%Copyright: -----------------------------------------------------------
%%EndComments

/CIDInit /ProcSet findresource begin

12 dict begin

begincmap

/CIDSystemInfo 3 dict dup begin
  /Registry (Adobe) def
  /Ordering (Japan1) def
  /Supplement 1 def
end def

/CMapName /83pv-RKSJ-H def
/CMapVersion 10.005 def
/CMapType 1 def

/UIDOffset 0 def
/XUID [1 10 25324] def

/WMode 0 def

5 begincodespacerange
  <00>   <80>
  <8140> <9FFC>
  <A0>   <DF>
  <E040> <FCFC>
  <FD>   <FF>
endcodespacerange

1 beginnotdefrange
<00> <1f> 1
endnotdefrange

100 begincidrange
<20> <7e>        1
<80> <80>       97
<8140> <817e>  633
<8180> <81ac>  696
<81b8> <81bf>  741
<81c8> <81ce>  749
<81da> <81e8>  756
<81f0> <81f7>  771
<81fc> <81fc>  779
<824f> <8258>  780
<8260> <8279>  790
<8281> <829a>  816
<829f> <82f1>  842
<8340> <837e>  925
<8380> <8396>  988
<839f> <83b6> 1011
<83bf> <83d6> 1035
<8440> <8460> 1059
<8470> <847e> 1092
<8480> <8491> 1107
<849f> <849f> 7479
<84a0> <84a0> 7481
<84a1> <84a1> 7491
<84a2> <84a2> 7495
<84a3> <84a3> 7503
<84a4> <84a4> 7499
<84a5> <84a5> 7507
<84a6> <84a6> 7523
<84a7> <84a7> 7515
<84a8> <84a8> 7531
<84a9> <84a9> 7539
<84aa> <84aa> 7480
<84ab> <84ab> 7482
<84ac> <84ac> 7494
<84ad> <84ad> 7498
<84ae> <84ae> 7506
<84af> <84af> 7502
<84b0> <84b0> 7514
<84b1> <84b1> 7530
<84b2> <84b2> 7522
<84b3> <84b3> 7538
<84b4> <84b4> 7554
<84b5> <84b5> 7511
<84b6> <84b6> 7526
<84b7> <84b7> 7519
<84b8> <84b8> 7534
<84b9> <84b9> 7542
<84ba> <84ba> 7508
<84bb> <84bb> 7527
<84bc> <84bc> 7516
<84bd> <84bd> 7535
<84be> <84be> 7545
<8540> <857e>  232
<8580> <8580>  390
<8581> <859e>  296
<859f> <85dd>  327
<85de> <85fc>  391
<8640> <867e>  422
<8680> <8691>  485
<8692> <8692>  295
<8693> <869e>  503
<86a2> <86ed> 7479
<8740> <875d> 7555
<875f> <8775> 7585
<8780> <878f> 7608
<8790> <8790>  762
<8791> <8791>  761
<8792> <8792>  769
<8793> <8799> 7624
<879a> <879a>  768
<879b> <879c> 7631
<889f> <88fc> 1125
<8940> <897e> 1219
<8980> <89fc> 1282
<8a40> <8a7e> 1407
<8a80> <8afc> 1470
<8b40> <8b7e> 1595
<8b80> <8bfc> 1658
<8c40> <8c7e> 1783
<8c80> <8cfc> 1846
<8d40> <8d7e> 1971
<8d80> <8dfc> 2034
<8e40> <8e7e> 2159
<8e80> <8efc> 2222
<8f40> <8f7e> 2347
<8f80> <8ffc> 2410
<9040> <907e> 2535
<9080> <90fc> 2598
<9140> <917e> 2723
<9180> <91fc> 2786
<9240> <927e> 2911
<9280> <92fc> 2974
<9340> <937e> 3099
<9380> <93fc> 3162
<9440> <947e> 3287
<9480> <94fc> 3350
<9540> <957e> 3475
<9580> <95fc> 3538
<9640> <967e> 3663
<9680> <96fc> 3726
endcidrange

100 begincidrange
<9740> <977e> 3851
<9780> <97fc> 3914
<9840> <9872> 4039
<989f> <98fc> 4090
<9940> <997e> 4184
<9980> <99fc> 4247
<9a40> <9a7e> 4372
<9a80> <9afc> 4435
<9b40> <9b7e> 4560
<9b80> <9bfc> 4623
<9c40> <9c7e> 4748
<9c80> <9cfc> 4811
<9d40> <9d7e> 4936
<9d80> <9dfc> 4999
<9e40> <9e7e> 5124
<9e80> <9efc> 5187
<9f40> <9f7e> 5312
<9f80> <9ffc> 5375
<a0> <df>      326
<e040> <e07e> 5500
<e080> <e0fc> 5563
<e140> <e17e> 5688
<e180> <e1fc> 5751
<e240> <e27e> 5876
<e280> <e2fc> 5939
<e340> <e37e> 6064
<e380> <e3fc> 6127
<e440> <e47e> 6252
<e480> <e4fc> 6315
<e540> <e57e> 6440
<e580> <e5fc> 6503
<e640> <e67e> 6628
<e680> <e6fc> 6691
<e740> <e77e> 6816
<e780> <e7fc> 6879
<e840> <e87e> 7004
<e880> <e8fc> 7067
<e940> <e97e> 7192
<e980> <e9fc> 7255
<ea40> <ea7e> 7380
<ea80> <eaa2> 7443
<eaa3> <eaa4> 8284
<eb40> <eb40>  633
<eb41> <eb42> 7887
<eb43> <eb4f>  636
<eb50> <eb51> 7889
<eb52> <eb5a>  651
<eb5b> <eb5d> 7891
<eb5e> <eb5f>  663
<eb60> <eb64> 7894
<eb65> <eb68>  670
<eb69> <eb7a> 7899
<eb7b> <eb7e>  692
<eb80> <eb80>  696
<eb81> <eb81> 7917
<eb82> <ebac>  698
<ebb8> <ebbf>  741
<ebc8> <ebce>  749
<ebda> <ebe8>  756
<ebf0> <ebf7>  771
<ebfc> <ebfc>  779
<ec4f> <ec58>  780
<ec60> <ec79>  790
<ec81> <ec9a>  816
<ec9f> <ec9f> 7918
<eca0> <eca0>  843
<eca1> <eca1> 7919
<eca2> <eca2>  845
<eca3> <eca3> 7920
<eca4> <eca4>  847
<eca5> <eca5> 7921
<eca6> <eca6>  849
<eca7> <eca7> 7922
<eca8> <ecc0>  851
<ecc1> <ecc1> 7923
<ecc2> <ece0>  877
<ece1> <ece1> 7924
<ece2> <ece2>  909
<ece3> <ece3> 7925
<ece4> <ece4>  911
<ece5> <ece5> 7926
<ece6> <eceb>  913
<ecec> <ecec> 7927
<eced> <ecf1>  920
<ed40> <ed40> 7928
<ed41> <ed41>  926
<ed42> <ed42> 7929
<ed43> <ed43>  928
<ed44> <ed44> 7930
<ed45> <ed45>  930
<ed46> <ed46> 7931
<ed47> <ed47>  932
<ed48> <ed48> 7932
<ed49> <ed61>  934
<ed62> <ed62> 7933
<ed63> <ed7e>  960
<ed80> <ed82>  988
<ed83> <ed83> 7934
<ed84> <ed84>  992
<ed85> <ed85> 7935
endcidrange

22 begincidrange
<ed86> <ed86>  994
<ed87> <ed87> 7936
<ed88> <ed8d>  996
<ed8e> <ed8e> 7937
<ed8f> <ed94> 1003
<ed95> <ed96> 7938
<ed9f> <edb6> 1011
<edbf> <edd6> 1035
<ee40> <ee5d> 7555
<ee5f> <ee6e> 7940
<ee6f> <ee75> 7601
<ee80> <ee81> 7956
<ee82> <ee8f> 7610
<ee90> <ee90>  762
<ee91> <ee91>  761
<ee92> <ee92>  769
<ee93> <ee99> 7624
<ee9a> <ee9a>  768
<ee9b> <ee9c> 7631
<fd> <fd>      152
<fe> <fe>      228
<ff> <ff>      124
endcidrange
endcmap
CMapName currentdict /CMap defineresource pop
end
end

%%EndResource
%%EOF";

        let mut cmap_parser = CmapParser::new(content.to_vec());
        let cmap = cmap_parser.parse().unwrap();
        // CID ranges include both endpoints; the former assertion omitted 222 upper bounds.
        assert_eq!(cmap.code_to_cids.len(), 7990_usize);
    }
}
