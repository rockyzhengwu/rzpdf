# Approval signature fixtures

These ASCII hex fixtures support offline v0.11 tests without OpenSSL or Python
at test runtime. They contain synthetic public certificates and signatures only;
no private key or production document is included. The generated document/test
code uses the repository license; the embedded Noto font in the workflow fixture
retains its separate OFL notice as described below.

`approval-seed.pdf.hex` decodes to a two-page PDF 2.0 with one invisible RSA
approval named `Seed`, produced by pyHanko 0.37.0 over a pikepdf 9.11.0 document.
Its decoded SHA-256 is
`db1379fe649a6666818c040bcc3fa344c896a7a4b09d18fa44fc5eb53eddf0be`.

`approval-second.cms.hex` decodes to a detached RSA/SHA-256 CMS response for
`prepare_additional_signature` on that exact seed, using field name `Second`
and the default 16 KiB reservation. OpenSSL signs the exact prepared ranges.
Its decoded SHA-256 is
`2a3a18a9e765f4bd8b3eac029f92f8fc2a87b05adacbc129de3bafe4aafa874e`.

Generation is implemented in `docs/check-v011-multisign.py`, in the
`fixture-seed.pdf` / `fixture-second.der` section. The checked-in pair comes
from the successful `v011-07-independent-second` run. New runs create fresh
disposable keys and therefore different fixture bytes; replace the pair
together if intentionally regenerating it. The disposable private key remains
in ignored local test output, not in the fixture directory.

The core verifies signature mathematics, not certificate trust or validity
dates. Tests neither access the network nor rely on the synthetic certificate
remaining unexpired. Independent gates use explicit local synthetic roots.

## Certified workflow fixtures

`workflow-certified.pdf.hex` decodes to the `docmdp-2.pdf` output of
`docs/check-v011-workflows.py` from `tmp/pdfs/v011-workflow-seventh`, whose 22
candidate workflow cases passed. It contains hierarchical empty signature fields,
text/checkbox fields and one synthetic pyHanko RSA certification signature, P=2.
Decoded SHA-256:
`05a4e6b71f3eacd64fabf802ec74cffba22cb4521758502ab1f216c5d609782b`.

`workflow-second.cms.hex` is the external OpenSSL CMS for `Sign.Second`, using
the default 16 KiB reservation and `SignatureAppearance` text `Approval only`
with default style. Decoded SHA-256:
`b92fa45a00e06a7bee79223ece514506468d569b0873d5b2241387dac3176ad0`.
Keep this pair together when regenerating; new keys/dates change its bytes.

The PDF embeds the existing Noto Sans SC static test subset from `../v03`.
Its upstream license and derivative provenance remain in
[the font fixture notice](../v03/README.md) and
[NotoSansSC OFL](../../../../docs/licenses/NotoSansSC-OFL.txt).
Embedding does not change that font's license. Disposable signer keys are never
part of these fixtures or required by Cargo tests.
