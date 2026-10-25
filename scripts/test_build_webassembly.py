"""WA-02: toolchain drift and host archives must never count as success."""
import contextlib
import io
import json
from pathlib import Path
import tempfile
import subprocess
import unittest
from unittest.mock import patch

import build_webassembly as build


class BuildTests(unittest.TestCase):
    def test_json_stdout_is_not_contaminated_by_warning_stderr(self):
        completed = subprocess.CompletedProcess(["cargo"], 0,
            stdout='{"packages": []}', stderr='warning: deprecated configuration\n')
        with patch.object(build.subprocess, "run", return_value=completed) as run, \
                contextlib.redirect_stderr(io.StringIO()) as diagnostics:
            self.assertEqual(build.capture_json(["cargo", "metadata"]), {"packages": []})
        self.assertIn("warning:", diagnostics.getvalue())
        self.assertTrue(run.call_args.kwargs["check"])

    def test_sdk_initialization_does_not_hide_or_weaken_version_check(self):
        self.assertTrue(build.matches_emcc_version("cache: INFO sanity checks\nemcc (Emscripten) 3.1.64 (revision)", "3.1.64"))
        self.assertFalse(build.matches_emcc_version("warning: expected 3.1.64 \nemcc (Emscripten) 3.1.65 (revision)", "3.1.64"))

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

    def test_optimized_archives_use_selected_profile(self):
        with tempfile.TemporaryDirectory() as folder:
            root = Path(folder)
            archive = root / build.EMSCRIPTEN / "wasm-release/build/test/out/libtest.a"
            archive.parent.mkdir(parents=True)
            archive.write_bytes(b"archive")
            with patch.object(build, "capture", return_value="test.o"), \
                 patch.object(build.subprocess, "check_output", return_value=b"\0asmtest"):
                items = build.archive_inventory(root, {}, "wasm-release")
            self.assertEqual(len(items), 1)
            self.assertIn("/wasm-release/", items[0]["path"])
            with self.assertRaisesRegex(ValueError, "no native archives"):
                build.archive_inventory(root, {}, "debug")

    def test_missing_archives_rejected(self):
        with tempfile.TemporaryDirectory() as folder:
            with self.assertRaisesRegex(ValueError, "no native archives"):
                build.archive_inventory(Path(folder), {})


if __name__ == "__main__":
    unittest.main()
