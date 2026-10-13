# v0.3 static font fixture

`NotoSansSC-static-subset.ttf` is a small static glyf TrueType subset for
Identity-H/Identity-V and font-edit tests. It is a modified derivative of the
Google Fonts Noto Sans SC font and retains its upstream SIL Open Font License
1.1 requirements; see `docs/licenses/NotoSansSC-OFL.txt`. It is not a general
CJK font and is not an installed or automatically discovered fallback.

- Upstream source: `https://github.com/google/fonts/tree/main/ofl/notosanssc`
- Original variable font SHA-256: `a3041811a78c361b1de50f953c805e0244951c21c5bd412f7232ef0d899af0da`
- Static instance SHA-256: `0516f3d1fb52efaf83f1d7bc540d779d1eb405f37e0dbb7696537444a0931087`
- This subset SHA-256: `cc677006b8f95153fe9bb59107ae7b201e71796181bffbaeb062a858d8a39942`
- FontTools version: 4.39.4
- Character repertoire: `你好，世界！Hello PDF竖排新增字符测试`

The static instance was created at wght=400 as described in
`docs/cjk-creation-validation.md`. From that instance, reproduce the fixture
with:

```sh
fonttools subset NotoSansSC-static.ttf \
  --text='你好，世界！Hello PDF竖排新增字符测试' \
  --output-file=NotoSansSC-static-subset.ttf \
  --notdef-glyph --notdef-outline --name-IDs='*' --name-languages='*'
```

The fixture retains `glyf`, `cmap`, `vhea`, and `vmtx`; its glyph repertoire
and hash are frozen for the v0.3 matrix. The original font and the larger
static instance are not included in this directory.

## Composite-glyph edit fixture

`RzpdfTestSans-composite.ttf` is a renamed, reduced derivative of Lato Regular,
used only for the `eacute` composite-glyph closure case. Lato's reserved font
name is not used as this modified font's family, full, unique or PostScript
name. Attribution and the upstream copyright remain in its name table; the
complete [Lato OFL notice](../../../../docs/licenses/Lato-OFL.txt) accompanies
it.

- Pinned upstream source: `https://raw.githubusercontent.com/google/fonts/66928797f271fed5ee428544e7229685e2d90127/ofl/lato/Lato-Regular.ttf`
- Source SHA-256: `d636e4683231f931eda222d588e944d082bfd3bdba02f928bee461c0f185b251`
- Renamed fixture SHA-256: `b400c05b3164fb6407b258b2438e27670dfb30ddb2cb9047cf7baecb91c3b534`
- FontTools version: 4.39.4
- Character repertoire: `Cafeé`; `é` is a composite glyph referencing `e`
  and an accent component.

Reproduce from the pinned source with:

```sh
fonttools subset Lato-Regular.ttf --text='Cafeé' \
  --output-file=Lato-reduced.ttf --notdef-glyph --notdef-outline \
  --name-IDs='*' --name-languages='*'
python3 docs/build-v03-lato-fixture.py \
  Lato-reduced.ttf RzpdfTestSans-composite.ttf
```

Use the same FontTools interpreter for both commands; the script must run from
the repository root. The intermediate `Lato-reduced.ttf` is not distributed.
