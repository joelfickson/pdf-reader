from pathlib import Path

PAGES = [
    ("The art of paying attention", "Begin with the page", [
        "There is a particular kind of quiet that arrives when we begin to read.",
        "A sentence takes the place of a notification. An idea has a little",
        "longer to settle.",
        "",
        "Good tools make this transition easy. They open the door, then step",
        "aside. They give us a clear page, comfortable space, and the freedom",
        "to find our own pace.",
    ]),
    ("Leave room in the margins", "Find your own pace", [
        "A page is more than the words printed on it. Space gives those words",
        "a rhythm. A generous margin creates a place to rest.",
        "",
        "Some pages invite a quick glance. Others ask us to slow down, move",
        "backward, or linger over a single thought.",
    ]),
    ("Keep what helps you read", "A place to return to", [
        "Open a document. Find your place. Make the words a little larger.",
        "Turn to the next page. These small actions form the foundation of",
        "a reading experience.",
        "",
        "Start with a page and a little room around it. Let everything else",
        "earn its place.",
    ]),
]


def escape(text):
    return text.replace("\\", "\\\\").replace("(", "\\(").replace(")", "\\)")


def add_outline(objects, page_refs, outline):
    root_id = len(objects) + 1
    objects.append(b"")

    def add_level(entries, parent_id):
        ids = [len(objects) + 1 + index for index in range(len(entries))]
        objects.extend([b""] * len(entries))
        for index, (title, page, children) in enumerate(entries):
            fields = [f"/Title ({escape(title)})", f"/Parent {parent_id} 0 R", f"/Dest [{page_refs[page]} /Fit]"]
            if index > 0:
                fields.append(f"/Prev {ids[index - 1]} 0 R")
            if index < len(entries) - 1:
                fields.append(f"/Next {ids[index + 1]} 0 R")
            if children:
                child_ids = add_level(children, ids[index])
                fields.append(f"/First {child_ids[0]} 0 R /Last {child_ids[-1]} 0 R /Count {len(child_ids)}")
            objects[ids[index] - 1] = f"<< {' '.join(fields)} >>".encode()
        return ids

    top = add_level(outline, root_id)
    objects[root_id - 1] = f"<< /Type /Outlines /First {top[0]} 0 R /Last {top[-1]} 0 R /Count {len(top)} >>".encode()
    objects[0] = f"<< /Type /Catalog /Pages 2 0 R /Outlines {root_id} 0 R /PageMode /UseOutlines >>".encode()


def add_links(objects, page_refs, links):
    annotations = {}
    for page, rect, target in links:
        if target[0] == "page":
            _, index, top = target
            action = f"/Dest [{page_refs[index]} /XYZ 0 {top} null]" if top is not None else f"/Dest [{page_refs[index]} /Fit]"
        else:
            action = f"/A << /S /URI /URI ({escape(target[1])}) >>"
        objects.append(f"<< /Type /Annot /Subtype /Link /Rect [{' '.join(str(value) for value in rect)}] /Border [0 0 0] {action} >>".encode())
        annotations.setdefault(page, []).append(f"{len(objects)} 0 R")
    for page, refs in annotations.items():
        page_id = int(page_refs[page].split()[0])
        body = objects[page_id - 1]
        objects[page_id - 1] = body[:-3] + f" /Annots [{' '.join(refs)}] >>".encode()


def create_pdf(destination, pages, rotations=(), blank_pages=(), outline=(), links=()):
    objects = [b"<< /Type /Catalog /Pages 2 0 R >>", b"", b"<< /Type /Font /Subtype /Type1 /BaseFont /Times-Roman /Encoding /WinAnsiEncoding >>", b"<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica /Encoding /WinAnsiEncoding >>"]
    children = []
    for index, (title, subtitle, lines) in enumerate(pages):
        page_id = len(objects) + 1
        stream_id = page_id + 1
        children.append(f"{page_id} 0 R")
        stream = ["0.16 0.18 0.17 rg", "BT /F2 9 Tf 64 724 Td (NOTES ON EVERYDAY THINGS) Tj ET", "0.82 0.83 0.80 RG 64 708 m 548 708 l S", f"BT /F1 30 Tf 64 632 Td ({escape(title)}) Tj ET", f"BT /F1 19 Tf 64 573 Td ({escape(subtitle)}) Tj ET", "BT /F1 13 Tf 21 TL 64 525 Td"]
        for line in lines:
            stream.append(f"({escape(line)}) Tj T*")
        stream.extend(["ET", "0.16 0.34 0.64 rg 64 248 80 5 re f", f"BT /F2 10 Tf 64 52 Td (Agentic PDF sample) Tj 460 0 Td ({index + 1}) Tj ET"])
        if index in blank_pages:
            stream = ["0.9 0.9 0.85 rg 60 400 490 300 re f", "0.4 0.5 0.6 rg 90 450 300 180 re f"]
        data = "\n".join(stream).encode("latin-1")
        rotation = rotations[index] if index < len(rotations) else 0
        objects.append(f"<< /Type /Page /Parent 2 0 R /MediaBox [0 0 612 792] /Rotate {rotation} /Resources << /Font << /F1 3 0 R /F2 4 0 R >> >> /Contents {stream_id} 0 R >>".encode())
        objects.append(f"<< /Length {len(data)} >>\nstream\n".encode() + data + b"\nendstream")
    objects[1] = f"<< /Type /Pages /Kids [{' '.join(children)}] /Count {len(children)} >>".encode()
    if outline:
        add_outline(objects, children, outline)
    if links:
        add_links(objects, children, links)
    output = bytearray(b"%PDF-1.4\n")
    offsets = [0]
    for index, body in enumerate(objects, 1):
        offsets.append(len(output))
        output.extend(f"{index} 0 obj\n".encode() + body + b"\nendobj\n")
    xref = len(output)
    output.extend(f"xref\n0 {len(offsets)}\n0000000000 65535 f \n".encode())
    for offset in offsets[1:]:
        output.extend(f"{offset:010d} 00000 n \n".encode())
    output.extend(f"trailer\n<< /Size {len(offsets)} /Root 1 0 R >>\nstartxref\n{xref}\n%%EOF\n".encode())
    destination.parent.mkdir(parents=True, exist_ok=True)
    destination.write_bytes(output)
    print(f"Created {destination}")


if __name__ == "__main__":
    create_pdf(Path(__file__).resolve().parent.parent / "tests/fixtures/reading.pdf", PAGES)

    create_pdf(
        Path(__file__).resolve().parent.parent / "tests/fixtures/selection.pdf",
        [
            ("Selection and copying", "A page for interaction tests", ["Café and naïve text keep their accents.", "Copy these lines in their original order.", "", "Returning to this tab should preserve your place."]),
            ("Rotated page", "Text coordinates follow the page", ["This page has a ninety-degree rotation.", "Select these words and copy them."]),
            ("", "", []),
        ],
        rotations=(0, 90, 0),
        blank_pages=(2,),
    )

    create_pdf(
        Path(__file__).resolve().parent.parent / "tests/fixtures/outline.pdf",
        PAGES,
        outline=[
            ("Part one: attention", 0, [("Begin with the page", 0, [])]),
            ("Part two: margins", 1, [("Leave room in the margins", 1, []), ("Find your own pace", 1, [])]),
            ("Keep what helps you read", 2, []),
        ],
    )

    create_pdf(
        Path(__file__).resolve().parent.parent / "tests/fixtures/links.pdf",
        [
            ("Links", "Internal and web links", ["Go to the last page", "Visit example.com"]),
            ("Rotated links", "Links follow the page rotation", ["Back to the first page"]),
            ("The last page", "A link target", ["", "", "", "", "", "", "", "", "", "You arrived here from a link."]),
        ],
        rotations=(0, 90, 0),
        links=[
            (0, [64, 520, 260, 540], ("page", 2, 340)),
            (0, [64, 499, 260, 519], ("uri", "https://example.com/")),
            (1, [64, 520, 260, 540], ("page", 0, None)),
        ],
    )
