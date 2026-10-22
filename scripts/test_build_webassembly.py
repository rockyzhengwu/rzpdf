"""WA-02: toolchain drift and host archives must never count as success."""
import contextlib
import io
import json
from pathlib import Path
import tempfile
import unittest
from unittest.mock import patch

import build_webassembly as build


class BuildTests(unittest.TestCase):
    def test_wrong_rust_revision_blocks_before_cargo(self):
        with tempfile.TemporaryDirectory() as folder:
            report = Path(folder) / "result.json"
            with patch.object(build, "sdk_environment", return_value={}), \
                 patch.object(build, "capture", return_value="commit-hash: wrong"), \
                 patch.object(build.subprocess, "call") as cargo, \
                 patch("sys.argv", ["build", "--output", str(report)]), \
                 contextlib.redirect_stdout(io.StringIO()):
                self.assertEqual(build.main(), 2)
            cargo.assert_not_called()
            self.assertFalse(json.loads(report.read_text())["p0_complete"])

    def test_host_archive_rejected(self):
        with tempfile.TemporaryDirectory() as folder:
            root = Path(folder)
            archive = root / build.EMSCRIPTEN / "debug/build/test/out/libtest.a"
            archive.parent.mkdir(parents=True)
            archive.write_bytes(b"archive")
            with patch.object(build, "capture", return_value="test.o"), \
                 patch.object(build.subprocess, "check_output", return_value=b"\x7fELF"):
                with self.assertRaisesRegex(ValueError, "non-Wasm"):
                    build.archive_inventory(root, {})

    def test_missing_archives_rejected(self):
        with tempfile.TemporaryDirectory() as folder:
            with self.assertRaisesRegex(ValueError, "no native archives"):
                build.archive_inventory(Path(folder), {})


if __name__ == "__main__":
    unittest.main()
