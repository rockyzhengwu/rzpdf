"""WA-T01 checks: missing prerequisites and failed builds cannot turn green."""
import contextlib
import io
import json
from pathlib import Path
import tempfile
import unittest
from unittest.mock import patch

import check_webassembly as probe


class PreflightTests(unittest.TestCase):
    def test_missing_target_or_compiler_blocks_without_building(self):
        tools = {name: dict(status="passed", path=name)
                 for name in ["rustc", "cargo", "rustup", "node", "emcc", "em++"]}
        tools["emcc"] = dict(status="blocked", reason="not on PATH")
        targets = dict(status="passed", output=probe.UNKNOWN + "\n")
        blockers = probe.prerequisite_blockers(tools, targets, probe.EMSCRIPTEN)
        self.assertEqual(len(blockers), 2)
        with patch.object(probe.subprocess, "run") as command:
            result = probe.build_check(probe.EMSCRIPTEN, tools, blockers, Path("unused.json"), 5)
        self.assertEqual(result["status"], "blocked")
        command.assert_not_called()

    def test_cli_reports_blocked_not_success_and_keeps_p0_incomplete(self):
        with tempfile.TemporaryDirectory() as folder:
            output = Path(folder) / "report.json"
            with patch.object(probe, "inspect_tools", return_value={"rustup": dict(status="blocked")}), \
                 patch.object(probe, "source_inventory", return_value={}), \
                 contextlib.redirect_stdout(io.StringIO()):
                code = probe.main(["--output", str(output), "--build-check"])
            report = json.loads(output.read_text())
            self.assertEqual(code, 2)
            self.assertEqual(report["core_check"]["status"], "blocked")
            self.assertFalse(report["p0_complete"])
            self.assertFalse(report["prerequisites_ready"])

    def test_failed_core_is_nonzero_even_when_tools_are_ready(self):
        with tempfile.TemporaryDirectory() as folder:
            output = Path(folder) / "report.json"
            tools = {"rustup": dict(status="passed", path="rustup")}
            with patch.object(probe, "inspect_tools", return_value=tools), \
                 patch.object(probe, "run", return_value=dict(status="passed", output=probe.EMSCRIPTEN)), \
                 patch.object(probe, "source_inventory", return_value={}), \
                 patch.object(probe, "build_check", return_value=dict(status="failed", returncode=101)), \
                 contextlib.redirect_stdout(io.StringIO()):
                code = probe.main(["--output", str(output), "--build-check"])
            report = json.loads(output.read_text())
            self.assertEqual(code, 1)
            self.assertTrue(report["prerequisites_ready"])
            self.assertFalse(report["p0_complete"])

    def test_ready_alternative_is_only_a_diagnostic_not_browser_acceptance(self):
        with tempfile.TemporaryDirectory() as folder:
            output = Path(folder) / "report.json"
            tools = {"rustup": dict(status="passed", path="rustup")}
            with patch.object(probe, "inspect_tools", return_value=tools), \
                 patch.object(probe, "run", return_value=dict(status="passed", output=probe.UNKNOWN)), \
                 patch.object(probe, "source_inventory", return_value={}), \
                 contextlib.redirect_stdout(io.StringIO()):
                code = probe.main(["--target", probe.UNKNOWN, "--output", str(output)])
            report = json.loads(output.read_text())
            self.assertEqual(code, 0)
            self.assertEqual(report["route"], "alternative_diagnostic")
            self.assertEqual(report["core_check"]["status"], "not_run")
            self.assertFalse(report["p0_complete"])

    def test_invalid_timeout_rejected_before_tool_probes(self):
        for value in ["0", "-1", "nan", "inf"]:
            with self.subTest(value=value), patch.object(probe, "inspect_tools") as tools, \
                 contextlib.redirect_stderr(io.StringIO()):
                with self.assertRaises(SystemExit) as stopped:
                    probe.main(["--output", "unused.json", "--timeout", value])
                self.assertEqual(stopped.exception.code, 2)
                tools.assert_not_called()


if __name__ == "__main__":
    unittest.main()
