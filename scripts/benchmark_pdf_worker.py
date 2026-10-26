#!/usr/bin/env python3
"""Isolated native engine adapter; stdin/stdout use the benchmark JSON protocol."""
import json
from pathlib import Path
import sys
import time


def memory_snapshot():
    import os
    import resource
    import subprocess
    try:
        rss = int(subprocess.check_output(["ps", "-o", "rss=", "-p", str(os.getpid())], text=True)) * 1024
    except (OSError, ValueError, subprocess.CalledProcessError):
        rss = None
    peak = resource.getrusage(resource.RUSAGE_SELF).ru_maxrss * (1 if sys.platform == "darwin" else 1024)
    return dict(rss_bytes=rss, peak_rss_bytes=peak)


def cpu_ms():
    import resource
    value = resource.getrusage(resource.RUSAGE_SELF)
    return (value.ru_utime + value.ru_stime) * 1000

def run(engine, job):
    # Imports initialize the engine before the first-document timer starts.
    if engine == "mupdf":
        import fitz
        version = fitz.VersionFitz
        binding_version = fitz.VersionBind
    else:
        import pypdfium2 as pdfium
        version = str(pdfium.PDFIUM_INFO)
        binding_version = str(pdfium.PYPDFIUM_INFO)
    diagnostics = job.get("diagnostics", False)
    initialized_memory = memory_snapshot() if diagnostics else None
    first_cpu_start = cpu_ms() if diagnostics else None
    dpi = job["dpi"]
    start = time.perf_counter()
    doc = fitz.open(job["path"]) if engine == "mupdf" else pdfium.PdfDocument(job["path"])
    count = min(len(doc), 5)
    open_ms = (time.perf_counter() - start) * 1000

    def prepare(index):
        if engine == "mupdf":
            page = doc.load_page(index)
            return page.get_displaylist(annots=False), page.rect
        page = doc[index]
        return page, page.get_size()

    def render(prepared):
        page, size = prepared
        if engine == "mupdf":
            rect = (size * (dpi / 72)).irect
            # Pixmap(cs, rect, alpha) in PyMuPDF 1.26.7 creates a non-null
            # FzSeparations(0), enabling separation conversion even with no
            # spot colors. Match DisplayList.get_pixmap's null separation set.
            pix = fitz.Pixmap('raw', fitz.mupdf.fz_new_pixmap_with_bbox(
                fitz.csRGB.this, fitz.mupdf.FzIrect(rect.x0, rect.y0, rect.x1, rect.y1),
                fitz.mupdf.FzSeparations(), 1))
            pix.clear_with(255)
            dev = fitz.DeviceWrapper(pix, None)
            fitz.mupdf.fz_run_display_list(
                page.this, dev.this, fitz.mupdf.FzMatrix(dpi / 72, 0, 0, dpi / 72, 0, 0),
                fitz.mupdf.FzRect(rect.x0, rect.y0, rect.x1, rect.y1), fitz.mupdf.FzCookie())
            fitz.mupdf.fz_close_device(dev.this)
            return pix.width, pix.height, pix.samples
        import math
        width, height = [math.ceil(v * dpi / 72) for v in size]
        bitmap = pdfium.PdfBitmap.new_native(width, height, pdfium.raw.FPDFBitmap_BGRA, rev_byteorder=True)
        try:
            pdfium.raw.FPDFBitmap_FillRect(bitmap, 0, 0, width, height, 0xffffffff)
            pdfium.raw.FPDF_RenderPageBitmap(bitmap, page, 0, 0, width, height, 0,
                                            pdfium.raw.FPDF_REVERSE_BYTE_ORDER)
            if bitmap.stride != width * 4:
                raise ValueError("unexpected bitmap stride")
            return width, height, bytes(bitmap.buffer)
        finally:
            bitmap.close()

    outputs = []

    def save(index, image):
        if job.get("output_dir"):
            path = Path(job["output_dir"]) / f"page-{index}.rgba"
            path.write_bytes(image[2])
            outputs.append(dict(page=index, width=image[0], height=image[1], file=str(path)))

    start = time.perf_counter()
    page = prepare(0)
    prepare_ms = (time.perf_counter() - start) * 1000
    start = time.perf_counter()
    image = render(page)
    raster_ms = (time.perf_counter() - start) * 1000
    first_cpu = cpu_ms() - first_cpu_start if diagnostics else None
    first_memory = memory_snapshot() if diagnostics else None
    width, height = image[:2]
    save(0, image)
    del image
    warm_ms = []
    replay_cpu_start = cpu_ms() if diagnostics else None
    replay_wall_start = time.perf_counter()
    for i in range(job["warmup"] + job["repeats"]):
        start = time.perf_counter()
        image = render(page)
        elapsed = (time.perf_counter() - start) * 1000
        del image
        if i >= job["warmup"]:
            warm_ms.append(elapsed)
    replay_wall_ms = (time.perf_counter() - replay_wall_start) * 1000
    replay_cpu = cpu_ms() - replay_cpu_start if diagnostics else None
    replay_memory = memory_snapshot() if diagnostics else None
    metrics = dict(initialized_memory=initialized_memory, after_first_memory=first_memory,
                   after_replay_memory=replay_memory, first_cpu_ms=first_cpu,
                   replay_cpu_ms=replay_cpu, replay_wall_ms=replay_wall_ms,
                   replay_iterations_including_warmup=job["warmup"] + job["repeats"]) if diagnostics else None
    next_ms = []
    for index in range(1, count):
        start = time.perf_counter()
        next_page = prepare(index)
        image = render(next_page)
        next_ms.append((time.perf_counter() - start) * 1000)
        save(index, image)
        del image
        if engine == "pdfium":
            next_page[0].close()
    if engine == "pdfium":
        page[0].close()
    doc.close()
    return dict(status="ok", engine=engine, version=version, binding_version=binding_version, open_ms=open_ms,
                prepare_ms=prepare_ms, raster_ms=raster_ms,
                first_ms=open_ms + prepare_ms + raster_ms, warm_ms=warm_ms, diagnostics=metrics,
                next_ms=next_ms, outputs=outputs, width=width, height=height)


if __name__ == "__main__":
    try:
        result = run(sys.argv[1], json.load(sys.stdin))
    except Exception as exc:
        result = dict(status="error", error=f"{type(exc).__name__}: {exc}")
    print(json.dumps(result))
