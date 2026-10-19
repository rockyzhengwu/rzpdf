// Original synthetic AF fixture. AGPL-3.0-only. Run with MuPDF mutool run.
// This is a producer recipe, not PDF JavaScript stored in the output document.
var doc = new PDFDocument();
var root = doc.getTrailer().get("Root");
root.put("Version", doc.newName("2.0"));
var file = doc.addEmbeddedFile("source.txt", "text/plain", "AF fixture\n", new Date("2020-01-01T00:00:00Z"), new Date("2020-01-01T00:00:00Z"), false);
file.put("AFRelationship", doc.newName("Source"));
file.put("Desc", doc.newString("Original MuPDF synthetic fixture"));
var links = doc.newArray();
links.push(file);
root.put("AF", doc.addObject(links));
root.put("OriginalAF", root.get("AF"));
var font = doc.addSimpleFont(new Font("Helvetica"));
var resources = doc.newDictionary();
var fonts = doc.newDictionary(); fonts.put("F1", font); resources.put("Font", fonts);
for (var i = 0; i < 2; ++i) {
    var page = doc.addPage([0, 0, 200, 200], 0, resources, "q 0 g 10 10 20 20 re f Q\nBT /F1 12 Tf 30 100 Td (AF fixture text) Tj ET\n");
    page.put("AF", links);
    doc.insertPage(-1, page);
}
doc.save(scriptArgs[0], "compress");
print("created original AF fixture " + scriptArgs[0]);
