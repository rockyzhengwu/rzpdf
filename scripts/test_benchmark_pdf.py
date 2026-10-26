import tempfile
from pathlib import Path
import unittest

from PIL import Image, ImageDraw
from benchmark_pdf import percentile95, quality, report, summarize


class BenchmarkTests(unittest.TestCase):
    def test_mupdf_adapter_matches_regular_device_rgb_rendering(self):
        try:
            import fitz
        except ImportError:
            self.skipTest("PyMuPDF is an optional benchmark dependency")
        from benchmark_pdf_worker import run
        with tempfile.TemporaryDirectory() as directory:
            doc = fitz.open()
            page = doc.new_page(width=100, height=100)
            page.draw_rect((10, 10, 90, 90), color=None, fill=(0.1, 0.3, 0.7))
            path = Path(directory) / "rgb.pdf"
            doc.save(path, no_new_id=True)
            expected = page.get_pixmap(alpha=False).pixel(50, 50)
            result = run("mupdf", dict(path=str(path), dpi=72, warmup=0, repeats=0, output_dir=directory))
            rgba = Path(result["outputs"][0]["file"]).read_bytes()
            self.assertEqual(tuple(rgba[(50*100+50)*4:(50*100+50)*4+4]), (*expected, 255))
            self.assertTrue(all(abs(a-b) <= 1 for a, b in zip(expected, (26,77,179))))
            doc.close()

    def test_tail_requires_enough_observations(self):
        self.assertIsNone(percentile95(list(range(19))))
        self.assertEqual(percentile95(list(range(1, 21))), 19)

    def test_phase_diagnostics_keep_rss_baseline_separate_from_peak(self):
        record = dict(case="test", dpi=72, engine="rzpdf", status="ok", first_ms=4,
                      warm_ms=[1], next_ms=[], worker_peak_rss_bytes=10*2**20,
                      diagnostics=dict(initialized_memory=dict(rss_bytes=2**20),
                                       after_first_memory=dict(rss_bytes=3*2**20),
                                       after_replay_memory=dict(rss_bytes=2*2**20),
                                       first_cpu_ms=2, replay_cpu_ms=8, replay_wall_ms=10))
        row = summarize([record], {("test",72): True})[0]
        self.assertEqual(row["initialized_rss_mib"],1)
        self.assertEqual(row["after_first_rss_mib"],3)
        self.assertEqual(row["peak_rss_mib"],10)
        self.assertEqual(row["first_cpu_wall_ratio"],0.5)
        self.assertEqual(row["replay_cpu_wall_ratio"],0.8)

    def test_quality_detects_missing_and_displaced_content(self):
        with tempfile.TemporaryDirectory() as directory:
            def output(name, offset=None):
                image = Image.new("RGBA", (100, 100), "white")
                if offset is not None:
                    ImageDraw.Draw(image).rectangle((offset, 20, offset + 30, 70), fill="black")
                path = Path(directory) / name
                path.write_bytes(image.tobytes())
                return dict(width=100, height=100, file=str(path))
            a = output("a", 20)
            self.assertTrue(quality(a, a, True)["passed"])
            self.assertFalse(quality(a, output("blank"), True)["passed"])
            self.assertFalse(quality(a, output("shift", 50), True)["passed"])
            blank = output("blank2")
            self.assertFalse(quality(blank, blank, True)["passed"])
            self.assertTrue(quality(blank, blank, False)["passed"])
            self.assertFalse(quality(a, dict(a, width=101), True)["passed"])
            Path(blank["file"]).write_bytes(Image.new("RGBA", (100, 100), (255, 255, 255, 0)).tobytes())
            self.assertFalse(quality(blank, blank, False)["passed"])

    def test_failed_trial_and_quality_gate_exclude_ratios(self):
        good = dict(case="test", dpi=72, engine="rzpdf", status="ok", first_ms=3,
                    warm_ms=[1, 2, 3], next_ms=[], worker_peak_rss_bytes=1048576)
        failure = dict(case="test", dpi=72, engine="rzpdf", status="timeout")
        row = summarize([good, failure], {("test", 72): True})[0]
        self.assertFalse(row["eligible"])
        self.assertIsNone(row["next_ms"])
        self.assertEqual(row["peak_rss_mib"], 1)
        self.assertFalse(summarize([good], {("test", 72): False})[0]["eligible"])

    def test_aggregate_requires_common_subset_and_excludes_baseline(self):
        records = []
        gates = {}
        for case, baseline in (("real", False), ("blank", True), ("failed", False)):
            gates[case, 72] = case != "failed"
            for engine in ("rzpdf", "mupdf", "pdfium"):
                value = (100 if baseline else 2) if engine == "rzpdf" else 1
                records.append(dict(case=case, dpi=72, engine=engine, baseline=baseline,
                                    status="ok", first_ms=value, warm_ms=[value], next_ms=[]))
        with tempfile.TemporaryDirectory() as directory:
            ratios = report(summarize(records, gates), gates, Path(directory))
        self.assertEqual(ratios["mupdf"]["first_ms"], dict(n=1, ratio=2.0))
        self.assertEqual(ratios["pdfium"]["next_ms"], dict(n=0, ratio=None))


if __name__ == "__main__":
    unittest.main()
