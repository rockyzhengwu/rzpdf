"""Regenerate pypdf-square.pdf from reportlab.pdf using pypdf 6.x."""

import sys
from pypdf import PdfReader, PdfWriter
from pypdf.generic import ArrayObject, DictionaryObject, FloatObject, NameObject

writer = PdfWriter()
writer.clone_document_from_reader(PdfReader(sys.argv[1]))
writer.add_named_destination("external-target", 1)
square = DictionaryObject({
    NameObject("/Type"): NameObject("/Annot"),
    NameObject("/Subtype"): NameObject("/Square"),
    NameObject("/Rect"): ArrayObject([FloatObject(300), FloatObject(480), FloatObject(450), FloatObject(540)]),
    NameObject("/C"): ArrayObject([FloatObject(0), FloatObject(0), FloatObject(1)]),
})
writer.add_annotation(page_number=0, annotation=square)
with open(sys.argv[2], "wb") as output:
    writer.write(output)
