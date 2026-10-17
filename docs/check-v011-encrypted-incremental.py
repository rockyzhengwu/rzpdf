"""R6 incremental gate using independent qpdf and MuPDF encryption producers.

All passwords and payloads are public synthetic test data. No downloads,
production credentials, Poppler, or fuzz are used.
"""
import argparse
import hashlib
import json
import os
from pathlib import Path
import subprocess
import pikepdf
from pikepdf import Array, Dictionary, Name, String

ROOT = Path(__file__).resolve().parents[1]


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("output", type=Path)
    out = parser.parse_args().output.resolve()
    out.mkdir(parents=True, exist_ok=False)
    binary = ROOT / "target/debug/examples/v011_encrypted_incremental"
    records = []

    def run(label, *args, password=None, success=True):
        environment = dict(os.environ)
        if password is not None:
            environment["RZPDF_INPUT_PASSWORD"] = password
        result = subprocess.run([str(a) for a in args], cwd=ROOT, env=environment,
                                capture_output=True, text=True)
        (out / f"{label}.log").write_text(result.stdout + result.stderr)
        if (result.returncode == 0) != success:
            raise RuntimeError((label, result.returncode, result.stdout, result.stderr))
        return result.stdout

    source = out / "plain.pdf"
    with pikepdf.Pdf.new() as pdf:
        page = pdf.add_blank_page(page_size=(200, 200)).obj
        page.Contents = pdf.make_stream(b"0 0 20 20 re f\n")
        font = pdf.make_indirect(Dictionary(Type=Name.Font, Subtype=Name.Type1, BaseFont=Name.Courier))
        page.Resources = Dictionary(Font=Dictionary(F1=font))
        page.PrivateString = String("old private object string")
        pdf.Root.Metadata = pdf.make_stream(b'<x:xmpmeta xmlns:x="adobe:ns:meta/"><rdf:RDF xmlns:rdf="http://www.w3.org/1999/02/22-rdf-syntax-ns#"><rdf:Description rdf:about=""/></rdf:RDF></x:xmpmeta>')
        pdf.Root.Metadata.Type, pdf.Root.Metadata.Subtype = Name.Metadata, Name.XML
        pdf.docinfo.Title = String("original-title")
        pdf.save(source, min_version="2.0")
    run("qpdf-version", "qpdf", "--version")
    run("mutool-version", "mutool", "-v")
    source17 = out / "plain-17-extension8.pdf"
    with pikepdf.open(source) as pdf:
        pdf.Root.Extensions = Dictionary(ADBE=Dictionary(BaseVersion=Name('/1.7'), ExtensionLevel=8))
        pdf.save(source17, force_version="1.7")

    variants = [
        ("qpdf-classic", ["qpdf", "--object-streams=disable", "--encrypt", "user", "owner", "256", "--", source]),
        ("qpdf-objstm", ["qpdf", "--object-streams=generate", "--encrypt", "user", "owner", "256", "--", source]),
        ("qpdf-clear-metadata", ["qpdf", "--encrypt", "user", "owner", "256", "--cleartext-metadata", "--", source]),
        ("mupdf-r6", ["mutool", "clean", "-E", "aes-256", "-U", "user", "-O", "owner", source]),
        ("qpdf-r6-17", ["qpdf", "--encrypt", "user", "owner", "256", "--", source17]),
        ("mupdf-r6-17", ["mutool", "clean", "-E", "aes-256", "-U", "user", "-O", "owner", source17]),
    ]
    for label, command in variants:
        encrypted = out / f"{label}-source.pdf"
        run(f"produce-{label}", *command, encrypted)
        with pikepdf.open(encrypted, password="owner") as pdf:
            assert pdf.encryption.R == 6 and pdf.encryption.V == 5
            protection = pdf.trailer.Encrypt.unparse(resolved=True)
            encryption_id = pdf.trailer.Encrypt.objgen
            first_id = bytes(pdf.trailer.ID[0])
            original_stream = pdf.pages[0].Contents.read_bytes()
            old_string = str(pdf.pages[0].PrivateString)
            info_id = pdf.trailer.Info.objgen[0]
            source_version = pdf.pdf_version
            if label == "qpdf-r6-17":
                assert source_version == "1.7"
                assert int(pdf.Root.Extensions.ADBE.ExtensionLevel) >= 8
        if label == "mupdf-r6-17":
            # This producer drops the extension declaration when saving 1.7.
            # Keep the original version boundary instead of silently upgrading.
            rejected = out / "missing-extension-output.pdf"
            run("reject-missing-extension", binary, encrypted, rejected, "not-supported", password="owner", success=False)
            assert "extension declaration" in (out / "reject-missing-extension.log").read_text()
            assert not rejected.exists()
            continue
        source_xref = run(f"source-xref-{label}", "qpdf", "--password=user", "--show-xref", encrypted)
        if label == "qpdf-objstm":
            assert f"{info_id}/0: compressed;" in source_xref
        for role in ["user", "owner"]:
            previous = encrypted
            for revision in range(1, 4):
                result = out / f"{label}-{role}-{revision}.pdf"
                title = f"{label}-{role}-{revision}-new-private-title"
                before = previous.read_bytes()
                before_hash = hashlib.sha256(before).hexdigest()
                with pikepdf.open(previous, password=role) as pdf:
                    previous_revision_id = bytes(pdf.trailer.ID[1])
                run(f"append-{label}-{role}-{revision}", binary, previous, result, title, password=role)
                after = result.read_bytes()
                assert after.startswith(before)
                assert hashlib.sha256(previous.read_bytes()).hexdigest() == before_hash
                assert b"incremental-private-payload" not in after[len(before):]
                with pikepdf.open(result, password="user") as pdf:
                    assert pdf.is_encrypted and pdf.encryption.R == 6
                    assert pdf.trailer.Encrypt.unparse(resolved=True) == protection
                    assert pdf.trailer.Encrypt.objgen == encryption_id
                    assert bytes(pdf.trailer.ID[0]) == first_id
                    assert bytes(pdf.trailer.ID[1]) != previous_revision_id
                    assert str(pdf.docinfo.Title) == title
                    assert pdf.pages[0].Contents.read_bytes() == original_stream
                    assert str(pdf.pages[0].PrivateString) == old_string
                    assert pdf.attachments[f"v011-{title}.bin"].get_file().read_bytes() == b"incremental-private-payload"
                run(f"qpdf-{label}-{role}-{revision}", "qpdf", "--password=user", "--check", result)
                # Independent decryption plus a no-page-render parse. The core
                # has not changed any paint program, so no pixel claim is made.
                run(f"mupdf-{label}-{role}-{revision}", "mutool", "show", "-p", "user", result, "trailer")
                records.append(dict(producer=label, authentication=role, revision=revision,
                                    pdf_version=source_version,
                                    encrypt_dictionary="direct" if encryption_id == (0, 0) else "indirect",
                                    source_sha256=before_hash, output_sha256=hashlib.sha256(after).hexdigest()))
                previous = result
        rejected = out / f"{label}-wrong-password.pdf"
        run(f"wrong-{label}", binary, encrypted, rejected, "must-not-save", password="wrong", success=False)
        assert not rejected.exists()
        occupied = out / f"{label}-user-1.pdf"
        digest = hashlib.sha256(occupied.read_bytes()).hexdigest()
        run(f"existing-{label}", binary, encrypted, occupied, "must-not-overwrite", password="owner", success=False)
        assert hashlib.sha256(occupied.read_bytes()).hexdigest() == digest

    # Legacy handlers are a separately frozen unsupported read decision, not a
    # fallback to an unauthenticated or incorrectly interpreted R6 handler.
    legacy = out / "legacy-r4.pdf"
    run("produce-r4", "qpdf", "--encrypt", "user", "owner", "128", "--use-aes=y", "--", source17, legacy)
    rejected = out / "legacy-r4-output.pdf"
    run("reject-r4", binary, legacy, rejected, "not-supported", password="owner", success=False)
    assert not rejected.exists()
    (out / "manifest.json").write_text(json.dumps(dict(cases=records,
        producers=["qpdf", "MuPDF"], legacy_read="R4 AESV2 explicitly rejected",
        unsupported_version="MuPDF 1.7 R6 without extension declaration explicitly rejected"), indent=2) + "\n")
    print(f"PASS {len(records)} encrypted incremental cases, 2 encryption producers, strict reopen, qpdf/MuPDF, refusals")


if __name__ == "__main__":
    main()
