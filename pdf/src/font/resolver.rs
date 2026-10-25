//! Backend-neutral substitute selection. Only proven Unicode enters this path;
//! PDF codes/CIDs/GIDs and PDF advances remain the caller's responsibility.
use crate::error::{PdfError, PdfResult};
use sha2::{Digest, Sha256};

pub const MAX_REGISTERED_FONTS: usize = 128;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct FontIdentity {
    pub sha256: [u8; 32],
    pub collection_index: u32,
}
impl FontIdentity {
    pub fn from_bytes(bytes: &[u8], collection_index: u32) -> Self {
        Self {
            sha256: Sha256::digest(bytes).into(),
            collection_index,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FontMetadata {
    pub family: String,
    pub aliases: Vec<String>,
    pub weight: u16,
    pub italic: bool,
    pub serif: bool,
    /// Explicit BCP-47 style language hints, not a promise of glyph coverage.
    pub languages: Vec<String>,
}
impl FontMetadata {
    pub fn validate(&self) -> PdfResult<()> {
        let valid = |s: &str, n| !s.is_empty() && s.len() <= n && !s.chars().any(char::is_control);
        if !valid(&self.family, 256)
            || !(1..=1000).contains(&self.weight)
            || self.aliases.len() > 16
            || self.languages.len() > 16
            || self.aliases.iter().any(|s| !valid(s, 256))
            || self.languages.iter().any(|s| !valid(s, 64))
        {
            return Err(PdfError::FontError(
                "invalid substitute font metadata".into(),
            ));
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Copy)]
pub struct FontRequest<'a> {
    pub name: &'a str,
    pub character: char,
    pub weight: u16,
    pub italic: bool,
    pub serif: bool,
    pub language: Option<&'a str>,
}
impl<'a> FontRequest<'a> {
    /// Descriptor flags override name heuristics when available. No code-to-
    /// Unicode guess is made here; the character must come from the PDF font.
    pub fn new(name: &'a str, character: char, flags: Option<u32>) -> Self {
        let lower: String = name
            .chars()
            .take(1024)
            .map(|c| c.to_ascii_lowercase())
            .collect();
        Self {
            name,
            character,
            weight: if flags.is_some_and(|f| f & (1 << 18) != 0) || lower.contains("bold") {
                700
            } else {
                400
            },
            italic: flags.map_or(lower.contains("italic") || lower.contains("oblique"), |f| {
                f & 64 != 0
            }),
            serif: flags.map_or(
                lower.contains("serif") && !lower.contains("sans") || lower.contains("times"),
                |f| f & 2 != 0,
            ),
            language: None,
        }
    }
}

/// Providers own validated faces and return GIDs in *that candidate's* face.
/// This interface neither downloads resources nor shares document font caches.
pub trait FontProvider {
    fn len(&self) -> usize;
    fn is_empty(&self) -> bool {
        self.len() == 0
    }
    fn metadata(&self, index: usize) -> &FontMetadata;
    fn identity(&self, index: usize) -> FontIdentity;
    fn glyph(&self, index: usize, character: char) -> Option<u16>;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ResolvedGlyph {
    pub provider_index: usize,
    pub identity: FontIdentity,
    pub glyph: u16,
}

fn normalized(name: &str) -> String {
    let name = if name.as_bytes().get(6) == Some(&b'+')
        && name.as_bytes()[..6].iter().all(u8::is_ascii_uppercase)
    {
        &name[7..]
    } else {
        name
    };
    name.chars()
        .filter(|c| c.is_alphanumeric())
        .flat_map(char::to_lowercase)
        .collect()
}

/// Bounded language hint. Source strings remain untouched when unusable.
pub fn valid_language(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 64
        && value.split('-').enumerate().all(|(i, p)| {
            !p.is_empty()
                && p.len() <= 8
                && p.bytes().all(|b| {
                    if i == 0 {
                        b.is_ascii_alphabetic()
                    } else {
                        b.is_ascii_alphanumeric()
                    }
                })
        })
}
/// Locale ranking shared by frontends: exact/prefix, compatible script,
/// unknown, then conflicting script or language. Never supplies Unicode.
pub fn language_rank(request: Option<&str>, candidate: &str) -> u8 {
    let Some(request) = request.filter(|s| valid_language(s)) else {
        return 0;
    };
    if !valid_language(candidate) {
        return 3;
    }
    if request.eq_ignore_ascii_case(candidate) {
        return 0;
    }
    let mut r = request.split('-');
    let mut c = candidate.split('-');
    if !r.next().unwrap().eq_ignore_ascii_case(c.next().unwrap()) {
        return 5;
    }
    let script = |s: &str| {
        s.split('-')
            .skip(1)
            .find(|p| p.len() == 4 && p.bytes().all(|b| b.is_ascii_alphabetic()))
            .map(str::to_ascii_lowercase)
    };
    let rs = script(request);
    let cs = script(candidate);
    if rs.is_some() && cs.is_some() && rs != cs {
        return 4;
    }
    let prefix = |a: &str, b: &str| {
        b.get(..a.len()).is_some_and(|p| p.eq_ignore_ascii_case(a))
            && b.as_bytes().get(a.len()) == Some(&b'-')
    };
    if prefix(request, candidate) || prefix(candidate, request) {
        1
    } else {
        2
    }
}

/// Selection preference, independent of the face's character coverage.
pub fn font_preference(metadata: &FontMetadata, request: FontRequest<'_>) -> PdfResult<[u16; 5]> {
    metadata.validate()?;
    if request.name.len() > 1024
        || request.language.is_some_and(|s| s.len() > 64)
        || !(1..=1000).contains(&request.weight)
    {
        return Err(PdfError::ResourceLimit("font preference input".into()));
    }
    let name = normalized(request.name);
    let named = normalized(&metadata.family) == name
        || metadata.aliases.iter().any(|a| normalized(a) == name);
    let language = if request.language.filter(|s| valid_language(s)).is_none() {
        0
    } else {
        metadata
            .languages
            .iter()
            .map(|l| language_rank(request.language, l))
            .min()
            .unwrap_or(3)
    };
    Ok([
        u16::from(!named),
        language as u16,
        u16::from(metadata.italic != request.italic),
        u16::from(metadata.serif != request.serif),
        metadata.weight.abs_diff(request.weight),
    ])
}

/// Stable ties follow registration order. Coverage is mandatory before name,
/// language or style ranking, so a same-name face without the glyph cannot win.
pub fn resolve(
    provider: &impl FontProvider,
    request: FontRequest<'_>,
) -> PdfResult<Option<ResolvedGlyph>> {
    if provider.len() > MAX_REGISTERED_FONTS
        || request.name.len() > 1024
        || request.language.is_some_and(|s| s.len() > 64)
        || !(1..=1000).contains(&request.weight)
    {
        return Err(PdfError::ResourceLimit("font resolver input".into()));
    }
    let mut best: Option<([u16; 5], ResolvedGlyph)> = None;
    for i in 0..provider.len() {
        let metadata = provider.metadata(i);
        metadata.validate()?;
        let Some(glyph) = provider.glyph(i, request.character).filter(|g| *g != 0) else {
            continue;
        };
        let rank = font_preference(metadata, request)?;
        if best.as_ref().is_none_or(|(old, _)| rank < *old) {
            best = Some((
                rank,
                ResolvedGlyph {
                    provider_index: i,
                    identity: provider.identity(i),
                    glyph,
                },
            ));
        }
    }
    Ok(best.map(|(_, value)| value))
}

#[cfg(test)]
mod tests {
    use super::*;
    type TestFace = (FontMetadata, FontIdentity, Vec<(char, u16)>);
    struct Provider(Vec<TestFace>);
    impl FontProvider for Provider {
        fn len(&self) -> usize {
            self.0.len()
        }
        fn metadata(&self, i: usize) -> &FontMetadata {
            &self.0[i].0
        }
        fn identity(&self, i: usize) -> FontIdentity {
            self.0[i].1
        }
        fn glyph(&self, i: usize, c: char) -> Option<u16> {
            self.0[i].2.iter().find(|(ch, _)| *ch == c).map(|(_, g)| *g)
        }
    }
    fn face(name: &str, bytes: &[u8], chars: Vec<(char, u16)>) -> TestFace {
        (
            FontMetadata {
                family: name.into(),
                aliases: vec![],
                weight: 400,
                italic: false,
                serif: false,
                languages: vec![],
            },
            FontIdentity::from_bytes(bytes, 0),
            chars,
        )
    }
    #[test]
    fn language_script_prefix_exact_and_unknown_are_ranked_without_cross_script_ties() {
        assert!(valid_language("zh-Hant-HK"));
        assert!(!valid_language("not valid"));
        assert!(!valid_language(&"a".repeat(65)));
        assert_eq!(language_rank(Some("JA-jp"), "ja-JP"), 0);
        assert!(
            language_rank(Some("zh-Hant-HK"), "zh-Hant")
                < language_rank(Some("zh-Hant-HK"), "zh-Hans")
        );
        let mut p = Provider(vec![
            face("Same", b"a", vec![('A', 1)]),
            face("Same", b"b", vec![('A', 2)]),
            face("Same", b"c", vec![('A', 3)]),
        ]);
        p.0[0].0.languages = vec!["zh-Hans".into()];
        p.0[1].0.languages = vec!["zh-Hant".into()];
        p.0[2].0.languages = vec!["ja-JP".into()];
        let mut q = FontRequest::new("Same", 'A', None);
        q.language = Some("zh-Hant-HK");
        assert_eq!(resolve(&p, q).unwrap().unwrap().glyph, 2);
        q.language = Some("ja");
        assert_eq!(resolve(&p, q).unwrap().unwrap().glyph, 3);
        q.language = None;
        assert_eq!(resolve(&p, q).unwrap().unwrap().glyph, 1);
        p.0[1].0.languages.clear();
        q.language = Some("zh-Hant");
        assert_eq!(resolve(&p, q).unwrap().unwrap().glyph, 2);
    }

    #[test]
    fn coverage_alias_subset_name_and_face_local_gid() {
        let mut p = Provider(vec![
            face("Source", b"a", vec![('A', 42), ('新', 0)]),
            face("CJK", b"b", vec![('新', 7)]),
            face("Other", b"c", vec![('新', 90)]),
        ]);
        p.0[1].0.aliases.push("Source".into());
        let x = resolve(&p, FontRequest::new("ABCDEF+Source", '新', None))
            .unwrap()
            .unwrap();
        assert_eq!((x.provider_index, x.glyph), (1, 7));
        assert_eq!(
            resolve(&p, FontRequest::new("Source", 'A', None))
                .unwrap()
                .unwrap()
                .glyph,
            42
        );
        assert_eq!(
            resolve(&p, FontRequest::new("Source", '缺', None)).unwrap(),
            None
        );
        assert_ne!(
            FontIdentity::from_bytes(b"a", 0),
            FontIdentity::from_bytes(b"a", 1)
        );
        assert_ne!(p.0[0].1, p.0[1].1);
    }
    #[test]
    fn style_language_stable_ties_and_bounded_metadata() {
        let mut p = Provider(vec![
            face("Sans", b"a", vec![('A', 1)]),
            face("Sans", b"b", vec![('A', 2)]),
        ]);
        p.0[1].0.weight = 700;
        p.0[1].0.languages.push("zh-Hans".into());
        let mut q = FontRequest::new("Unknown-Bold", 'A', None);
        q.language = Some("zh-CN");
        assert_eq!(resolve(&p, q).unwrap().unwrap().provider_index, 1);
        q.language = None;
        q.weight = 400;
        assert_eq!(resolve(&p, q).unwrap().unwrap().provider_index, 0);
        p.0[0].0.family = "x".repeat(257);
        assert!(resolve(&p, q).is_err());
    }
}
