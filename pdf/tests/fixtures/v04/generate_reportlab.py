"""Regenerate reportlab.pdf: python3 generate_reportlab.py OUTPUT.pdf.

ReportLab is only a fixture-generation dependency, not a project dependency.
The fixture is created from this script and contains no copied PDF source.
"""

import sys
from reportlab.pdfgen import canvas
from reportlab.lib.pagesizes import letter

c = canvas.Canvas(sys.argv[1], pagesize=letter, pageCompression=0)
c.bookmarkPage("first")
c.addOutlineEntry("External first", "first")
c.setFont("Helvetica", 12)
c.drawString(60, 740, "External navigation and AcroForm sample")
c.linkURL("https://example.org/", (60, 700, 240, 725), relative=0)
c.linkRect("", "second", (60, 660, 240, 685), relative=0)
c.acroForm.textfield(name="external_text", tooltip="External text", value="External",
                     x=60, y=580, width=220, height=28, forceBorder=True)
c.acroForm.checkbox(name="external_check", tooltip="External checkbox", checked=True,
                    x=60, y=530, size=24, forceBorder=True)
c.showPage()
c.bookmarkPage("second")
c.addOutlineEntry("External second", "second")
c.drawString(60, 740, "Second page")
c.save()
