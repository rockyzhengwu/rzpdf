use std::collections::HashMap;

use crate::error::{PdfError, PdfResult};

#[derive(Debug, Default, Clone)]
pub struct CharMetrics {
    code: i32,
    wx: f32,
    bbox: [f32; 4],
}

#[derive(Debug, Default, Clone)]
pub struct Afm {
    font_name: String,
    family_name: String,
    char_metrics: HashMap<String, CharMetrics>,
    budget_metrics: Option<Vec<(String, CharMetrics)>>,
}
impl Afm {
    pub(crate) fn cache_bytes(&self) -> usize {
        self.font_name
            .capacity()
            .saturating_add(self.family_name.capacity())
            .saturating_add(
                self.char_metrics
                    .capacity()
                    .saturating_mul(std::mem::size_of::<(String, CharMetrics)>() + 16)
                    .saturating_mul(2),
            )
            .saturating_add(
                self.char_metrics
                    .keys()
                    .map(String::capacity)
                    .sum::<usize>(),
            )
            .saturating_add(self.budget_metrics.as_ref().map_or(0, |v| {
                v.capacity()
                    .saturating_mul(std::mem::size_of::<(String, CharMetrics)>())
                    .saturating_add(v.iter().map(|(k, _)| k.capacity()).sum::<usize>())
            }))
    }

    pub fn get_char_width(&self, name: &str) -> Option<f32> {
        if let Some(entries) = &self.budget_metrics {
            return entries
                .binary_search_by(|(key, _)| key.as_str().cmp(name))
                .ok()
                .map(|index| entries[index].1.wx);
        }
        if let Some(cm) = self.char_metrics.get(name) {
            return Some(cm.wx);
        }
        None
    }
    #[cfg(test)]
    pub fn get_char_width_with_code(&self, code: u8) -> Option<f32> {
        if let Some(entries) = &self.budget_metrics {
            return entries
                .iter()
                .find(|(_, metric)| metric.code == i32::from(code))
                .map(|(_, metric)| metric.wx);
        }
        for cm in self.char_metrics.values() {
            if cm.code == code as i32 {
                return Some(cm.wx);
            }
        }
        None
    }
}

fn parse_line(line: &str) -> Option<(&str, std::str::SplitAsciiWhitespace<'_>)> {
    let mut words = line.split_ascii_whitespace();
    let key = words.next()?;
    Some((key, words))
}

fn parse_char_metric(line: &str) -> Option<(&str, CharMetrics)> {
    let line = line.trim();
    if line.is_empty() {
        None
    } else {
        let items = line.split(';');
        let mut char_matrics = CharMetrics::default();
        let mut name = "";
        for item in items {
            let item = item.trim();
            if let Some((key, mut values)) = parse_line(item) {
                match key {
                    "C" => {
                        let code = values.next()?.parse::<i32>().ok()?;
                        char_matrics.code = code;
                    }
                    "WX" => {
                        let wx = values.next()?.parse::<f32>().ok()?;
                        if !wx.is_finite() {
                            return None;
                        }
                        char_matrics.wx = wx;
                    }
                    "N" => {
                        name = values.next()?;
                    }
                    "B" => {
                        let lx = values.next()?.parse::<f32>().ok()?;
                        let ly = values.next()?.parse::<f32>().ok()?;
                        let ux = values.next()?.parse::<f32>().ok()?;
                        let uy = values.next()?.parse::<f32>().ok()?;
                        if ![lx, ly, ux, uy].iter().all(|v| v.is_finite()) {
                            return None;
                        }
                        char_matrics.bbox = [lx, ly, ux, uy];
                    }
                    _ => {}
                }
            }
        }
        if name.is_empty() {
            return None;
        }
        Some((name, char_matrics))
    }
}

#[cfg(test)]
pub fn parse_afm(content: String) -> PdfResult<Afm> {
    parse_afm_with_limits(content, &crate::parser::options::Limits::default())
}

#[cfg(test)]
pub fn parse_afm_with_limits(
    content: String,
    limits: &crate::parser::options::Limits,
) -> PdfResult<Afm> {
    parse_afm_str_with_limits(&content, limits)
}

pub fn parse_afm_str_with_limits(
    content: &str,
    limits: &crate::parser::options::Limits,
) -> PdfResult<Afm> {
    parse_afm_str_inner(content, limits, None).map(|(afm, _)| afm)
}

pub(crate) fn parse_afm_str_with_budget(
    content: &str,
    limits: &crate::parser::options::Limits,
    budget: &crate::budget::OperationBudget,
) -> PdfResult<(Afm, crate::budget::MemoryLease)> {
    let (afm, lease) = parse_afm_str_inner(content, limits, Some(budget))?;
    Ok((afm, lease.unwrap()))
}

fn parse_afm_str_inner(
    content: &str,
    limits: &crate::parser::options::Limits,
    budget: Option<&crate::budget::OperationBudget>,
) -> PdfResult<(Afm, Option<crate::budget::MemoryLease>)> {
    if content.len() > limits.max_decoded_bytes {
        return Err(PdfError::ResourceLimit("AFM input bytes".into()));
    }
    let mut lines = content.split('\n');
    let mut afm = Afm {
        budget_metrics: budget.map(|_| Vec::new()),
        ..Default::default()
    };
    let mut table_lease = budget.map(|pool| pool.reserve(0)).transpose()?;
    let mut font_name_lease = budget.map(|pool| pool.reserve(0)).transpose()?;
    let mut family_name_lease = budget.map(|pool| pool.reserve(0)).transpose()?;
    let mut metric_name_lease = budget.map(|pool| pool.reserve(0)).transpose()?;
    let copy_name = |name: &str| -> PdfResult<(String, Option<crate::budget::MemoryLease>)> {
        if let Some(pool) = budget {
            let (name, lease) = crate::budget::copy_string(name, pool)?;
            Ok((name, Some(lease)))
        } else {
            Ok((name.to_string(), None))
        }
    };
    let mut declared_metrics = 0usize;
    while let Some(line) = lines.next() {
        if let Some((key, mut values)) = parse_line(line) {
            match key {
                "FontName" => {
                    let fname = values
                        .next()
                        .ok_or_else(|| PdfError::FontError("missing AFM FontName".into()))?;
                    let (fname, lease) = copy_name(fname)?;
                    afm.font_name = fname;
                    font_name_lease = lease;
                }
                "FamilyName" => {
                    let fname = values
                        .next()
                        .ok_or_else(|| PdfError::FontError("missing AFM FamilyName".into()))?;
                    let (fname, lease) = copy_name(fname)?;
                    afm.family_name = fname;
                    family_name_lease = lease;
                }
                "CharWidth" => {}
                "StartCharMetrics" => {
                    let n = values
                        .next()
                        .ok_or(PdfError::FontError(
                            "Afm charactor metrics number is none".to_string(),
                        ))?
                        .parse::<u32>()
                        .map_err(|_| PdfError::FontError("invalid AFM metric count".into()))?;
                    declared_metrics = declared_metrics
                        .checked_add(n as usize)
                        .filter(|&count| count <= limits.max_objects)
                        .ok_or_else(|| PdfError::ResourceLimit("AFM metric entries".into()))?;
                    let mut i = 0;
                    while i < n {
                        if let Some(nl) = lines.next() {
                            if nl.trim().is_empty() {
                                continue;
                            }
                            if let Some((name, char_matrics)) = parse_char_metric(nl) {
                                let (name, name_lease) = copy_name(name)?;
                                if let Some(pool) = budget {
                                    let entries = afm.budget_metrics.as_mut().unwrap();
                                    match entries.binary_search_by(|(key, _)| key.cmp(&name)) {
                                        Ok(index) => entries[index].1 = char_matrics,
                                        Err(index) => {
                                            if entries.len() == entries.capacity() {
                                                let capacity = entries
                                                    .capacity()
                                                    .checked_mul(2)
                                                    .unwrap_or(limits.max_objects)
                                                    .max(2)
                                                    .min(limits.max_objects);
                                                let size =
                                                    std::mem::size_of::<(String, CharMetrics)>();
                                                let mut lease = pool.reserve(
                                                    capacity.checked_mul(size).ok_or_else(
                                                        || {
                                                            PdfError::ResourceLimit(
                                                                "AFM table capacity".into(),
                                                            )
                                                        },
                                                    )?,
                                                )?;
                                                let mut next = Vec::new();
                                                next.try_reserve_exact(capacity).map_err(|_| {
                                                    PdfError::ResourceLimit(
                                                        "AFM table allocation".into(),
                                                    )
                                                })?;
                                                lease.resize(
                                                    next.capacity().checked_mul(size).ok_or_else(
                                                        || {
                                                            PdfError::ResourceLimit(
                                                                "AFM table capacity".into(),
                                                            )
                                                        },
                                                    )?,
                                                )?;
                                                next.append(entries);
                                                *entries = next;
                                                table_lease = Some(lease);
                                            }
                                            entries.insert(index, (name, char_matrics));
                                        }
                                    }
                                } else {
                                    afm.char_metrics.try_reserve(1).map_err(|_| {
                                        PdfError::ResourceLimit("AFM metric allocation".into())
                                    })?;
                                    afm.char_metrics.insert(name, char_matrics);
                                }
                                if let Some(mut lease) = name_lease {
                                    metric_name_lease.as_mut().unwrap().absorb(&mut lease)?;
                                }
                                i += 1;
                            } else {
                                return Err(PdfError::FontError(
                                    "invalid AFM character metric".into(),
                                ));
                            }
                        } else {
                            return Err(PdfError::FontError(
                                "truncated AFM character metrics".into(),
                            ));
                        }
                    }
                }
                _ => {}
            }
        }
    }
    if let Some(mut family) = family_name_lease {
        font_name_lease.as_mut().unwrap().absorb(&mut family)?;
    }
    if let Some(mut names) = metric_name_lease {
        font_name_lease.as_mut().unwrap().absorb(&mut names)?;
    }
    if let Some(mut table) = table_lease {
        font_name_lease.as_mut().unwrap().absorb(&mut table)?;
    }
    Ok((afm, font_name_lease))
}

#[cfg(test)]
mod tests {
    use font_data::get_builtin_font_matrices;

    use super::parse_afm;

    #[test]
    fn budget_table_growth_accounts_for_old_and_new_capacity() {
        use crate::{budget::OperationBudget, parser::options::Limits};
        let source = "StartCharMetrics 4\nC 65 ; WX 500 ; N A ;\nC 66 ; WX 600 ; N B ;\nC 67 ; WX 700 ; N C ;\nC 65 ; WX 800 ; N A ;\n";
        let size = std::mem::size_of::<(String, super::CharMetrics)>();
        let pool = OperationBudget::new(6 * size + 3);
        let neighbor = pool.reserve(1).unwrap();
        assert!(super::parse_afm_str_with_budget(source, &Limits::default(), &pool).is_err());
        assert_eq!(pool.live_bytes(), 1);
        drop(neighbor);
        let (afm, lease) =
            super::parse_afm_str_with_budget(source, &Limits::default(), &pool).unwrap();
        assert!(afm.char_metrics.is_empty());
        assert_eq!(afm.budget_metrics.as_ref().unwrap().len(), 3);
        assert_eq!(afm.get_char_width("A"), Some(800.0));
        assert_eq!(afm.get_char_width_with_code(65), Some(800.0));
        assert_eq!(pool.peak_bytes(), 6 * size + 3);
        assert_eq!(pool.live_bytes(), 4 * size + 4);
        drop(afm);
        drop(lease);
        assert_eq!(pool.live_bytes(), 0);
    }

    #[test]
    fn metric_name_budget_rejects_and_releases_invalid_records() {
        use crate::{budget::OperationBudget, parser::options::Limits};
        let source = "StartCharMetrics 2\nC 65 ; WX 500 ; N A ;\nC 66 ; WX 600 ; N B ;\n";
        let pool =
            OperationBudget::new(2 + 2 * std::mem::size_of::<(String, super::CharMetrics)>());
        let neighbor = pool.reserve(1).unwrap();
        assert!(super::parse_afm_str_with_budget(source, &Limits::default(), &pool).is_err());
        assert_eq!(pool.live_bytes(), 1);
        drop(neighbor);
        let (afm, lease) =
            super::parse_afm_str_with_budget(source, &Limits::default(), &pool).unwrap();
        assert_eq!(afm.get_char_width("A"), Some(500.0));
        assert_eq!(afm.get_char_width("B"), Some(600.0));
        assert_eq!(pool.live_bytes(), pool.maximum_bytes());
        drop(afm);
        drop(lease);
        assert_eq!(pool.live_bytes(), 0);
        assert!(
            super::parse_afm_str_with_budget(
                "StartCharMetrics 2\nC 65 ; WX 500 ; N A ;\nC 66 ; WX NaN ; N B ;\n",
                &Limits::default(),
                &pool
            )
            .is_err()
        );
        assert_eq!(pool.live_bytes(), 0);
    }

    #[test]
    fn header_budget_retains_final_names_and_charges_replacement_peak() {
        use crate::{budget::OperationBudget, parser::options::Limits};
        let source = "FontName A\nFamilyName BC\nFontName DEF\n";
        let pool = OperationBudget::new(6);
        let neighbor = pool.reserve(1).unwrap();
        assert!(super::parse_afm_str_with_budget(source, &Limits::default(), &pool).is_err());
        assert_eq!(pool.live_bytes(), 1);
        drop(neighbor);
        let (afm, lease) =
            super::parse_afm_str_with_budget(source, &Limits::default(), &pool).unwrap();
        assert_eq!(afm.font_name, "DEF");
        assert_eq!(afm.family_name, "BC");
        assert_eq!(pool.peak_bytes(), 6);
        assert_eq!(pool.live_bytes(), 5);
        drop(afm);
        drop(lease);
        assert_eq!(pool.live_bytes(), 0);
        assert!(
            super::parse_afm_str_with_budget("FontName A\nFamilyName\n", &Limits::default(), &pool)
                .is_err()
        );
        assert_eq!(pool.live_bytes(), 0);
    }

    #[test]
    fn metric_name_moves_to_key_and_duplicate_names_keep_last_metric() {
        let afm = parse_afm("StartCharMetrics 3\nC 65 ; WX 500 ; N A ; B 0 0 1 1 ;\nC 66 ; WX 600 ; N B ;\nC 65 ; WX 700 ; N ignored ; N A ;\nEndCharMetrics\n".into()).unwrap();
        assert_eq!(afm.char_metrics.len(), 2);
        assert_eq!(afm.get_char_width("A"), Some(700.0));
        assert_eq!(afm.get_char_width_with_code(65), Some(700.0));
        assert_eq!(afm.get_char_width("B"), Some(600.0));
        assert_eq!(afm.get_char_width("ignored"), None);
        let copy = afm.clone();
        assert_eq!(copy.get_char_width("A"), Some(700.0));
    }

    #[test]
    fn all_standard_font_metrics_remain_readable() {
        for name in [
            "Courier",
            "Courier-Bold",
            "Courier-BoldOblique",
            "Courier-Oblique",
            "Helvetica",
            "Helvetica-Bold",
            "Helvetica-BoldOblique",
            "Helvetica-Oblique",
            "Times-Roman",
            "Times-Bold",
            "Times-Italic",
            "Times-BoldItalic",
            "Symbol",
            "ZapfDingbats",
        ] {
            let input = get_builtin_font_matrices(name).unwrap();
            let parsed = parse_afm(String::from_utf8(input.to_vec()).unwrap()).unwrap();
            assert!(!parsed.char_metrics.is_empty(), "{name}");
            let pool = crate::budget::OperationBudget::new(262144);
            let (budgeted, lease) = super::parse_afm_str_with_budget(
                std::str::from_utf8(input).unwrap(),
                &crate::parser::options::Limits::default(),
                &pool,
            )
            .unwrap();
            assert_eq!(budgeted.font_name, parsed.font_name, "{name}");
            assert_eq!(budgeted.family_name, parsed.family_name, "{name}");
            let entries = budgeted.budget_metrics.as_ref().unwrap();
            assert_eq!(entries.len(), parsed.char_metrics.len(), "{name}");
            for (glyph, expected) in &parsed.char_metrics {
                let index = entries.binary_search_by(|(key, _)| key.cmp(glyph)).unwrap();
                let actual = &entries[index].1;
                assert_eq!(actual.code, expected.code, "{name}/{glyph}");
                assert_eq!(actual.wx, expected.wx, "{name}/{glyph}");
                assert_eq!(actual.bbox, expected.bbox, "{name}/{glyph}");
                assert_eq!(
                    budgeted.get_char_width(glyph),
                    parsed.get_char_width(glyph),
                    "{name}/{glyph}"
                );
            }
            for code in 0..=255u8 {
                assert_eq!(
                    budgeted.get_char_width_with_code(code),
                    parsed.get_char_width_with_code(code),
                    "{name}/{code}"
                );
            }
            let expected_bytes = budgeted.font_name.capacity()
                + budgeted.family_name.capacity()
                + entries.capacity() * std::mem::size_of::<(String, super::CharMetrics)>()
                + entries.iter().map(|(key, _)| key.capacity()).sum::<usize>();
            assert_eq!(pool.live_bytes(), expected_bytes, "{name}");
            assert_eq!(lease.bytes(), expected_bytes, "{name}");
            drop(budgeted);
            drop(lease);
            assert_eq!(pool.live_bytes(), 0, "{name}");
        }
    }

    #[test]
    fn afm_budget_checks_input_and_cumulative_declared_entries() {
        use crate::parser::options::Limits;
        let input = "StartCharMetrics 1\nC 1 ; WX 600 ; N A ; B 0 0 1 1 ;";
        let limits = Limits {
            max_objects: 1,
            max_decoded_bytes: input.len(),
            ..Default::default()
        };
        assert!(super::parse_afm_with_limits(input.into(), &limits).is_ok());
        for (source, limits) in [
            (
                input.to_string(),
                Limits {
                    max_decoded_bytes: input.len() - 1,
                    ..limits
                },
            ),
            ("StartCharMetrics 4294967295".into(), limits),
            (
                format!("{input}\n{input}"),
                Limits {
                    max_decoded_bytes: usize::MAX,
                    ..limits
                },
            ),
        ] {
            assert!(matches!(
                super::parse_afm_with_limits(source, &limits),
                Err(crate::error::PdfError::ResourceLimit(_))
            ));
        }
    }

    #[test]
    fn malformed_afm_returns_errors_without_panicking_or_looping() {
        for source in [
            "FontName",
            "FamilyName",
            "StartCharMetrics nope",
            "StartCharMetrics 1",
            "StartCharMetrics 2\nC 1 ; WX 600 ; N A ; B 0 0 1 1 ;",
            "StartCharMetrics 1\nC ; N A ;",
            "StartCharMetrics 1\nC nope ; N A ;",
            "StartCharMetrics 1\nWX NaN ; N A ;",
            "StartCharMetrics 1\nN ;",
            "StartCharMetrics 1\nB 0 1 ; N A ;",
            "StartCharMetrics 1\nB 0 1 inf 2 ; N A ;",
            "StartCharMetrics 1\nEndCharMetrics",
        ] {
            assert!(
                matches!(
                    parse_afm(source.into()),
                    Err(crate::error::PdfError::FontError(_))
                ),
                "{source}"
            );
        }
    }

    #[test]
    fn test_afm() {
        let afm_data = get_builtin_font_matrices("Courier").unwrap();
        let s = String::from_utf8(afm_data.to_vec()).unwrap();
        let afm = parse_afm(s).unwrap();
        assert_eq!(315, afm.char_metrics.len());
    }
}
