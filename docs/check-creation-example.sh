#!/usr/bin/env bash
# Read-only external checks for pdf/examples/create_document.rs output.
set -euo pipefail

if [[ $# -ne 1 ]]; then
    echo 'Usage: bash docs/check-creation-example.sh path/to/example.pdf' >&2
    exit 2
fi
task_pdf_path=$1
for task_tool in qpdf mutool jq xmllint; do
    command -v "$task_tool" >/dev/null || {
        echo "Missing external tool: $task_tool" >&2
        exit 2
    }
done
qpdf --check "$task_pdf_path"
qpdf --json --json-key=qpdf "$task_pdf_path" | jq -e '
    .qpdf[1] as $objects |
    $objects.trailer.value as $trailer |
    $objects["obj:" + $trailer["/Info"]].value as $info |
    $objects["obj:" + $trailer["/Root"]].value as $root |
    $objects["obj:" + $root["/Metadata"]].stream.dict as $metadata |
    $objects["obj:" + $root["/Pages"]].value as $pages |
    ($info["/Title"] == "u:rzpdf 构建示例") and
    ($info["/Author"] == "u:rzpdf") and
    ($info["/Creator"] == "u:rzpdf create_document example") and
    ($metadata["/Type"] == "/Metadata") and
    ($metadata["/Subtype"] == "/XML") and
    ($pages["/Count"] == 2)
' >/dev/null
mutool show -b "$task_pdf_path" trailer/Root/Metadata | xmllint --nonet --noout -
task_author=$(mutool show -b "$task_pdf_path" trailer/Root/Metadata | xmllint --nonet --xpath \
    'string(//*[namespace-uri()="http://purl.org/dc/elements/1.1/" and local-name()="creator"]/*[namespace-uri()="http://www.w3.org/1999/02/22-rdf-syntax-ns#" and local-name()="Seq"]/*[namespace-uri()="http://www.w3.org/1999/02/22-rdf-syntax-ns#" and local-name()="li"][1])' -)
[[ "$task_author" == rzpdf ]] || { echo 'Unexpected XMP creator' >&2; exit 1; }
task_text=$(mutool draw -F txt "$task_pdf_path")
[[ "$task_text" == *'Page 1 | Text, image and metadata'* && "$task_text" == *'Page 2 | Text, image and metadata'* ]] || {
    echo 'Expected page text was not extracted' >&2
    exit 1
}
echo 'Example structure, Unicode Info, Metadata XML/creator and page text checks passed.'
