"""QA for the built PDF: does every ledger value reach the page, and does anything spill
over the margin? Reads tex/numbers.tex (the macro list), the extracted PDF text and the
page geometry. Report-only; the pass/fail gate for the paper is verify_all.sh."""
import pathlib, re, sys
import fitz

pdf_path = sys.argv[1]
doc = fitz.open(pdf_path)

text = "\n".join(page.get_text() for page in doc)

# 1. undefined-macro markers must not appear
bad = [m for m in re.findall(r"\?\?[^?\s]+\?\?", text)]
print("undefined macro markers:", len(bad), bad[:5])

# 2. margin overflow: any span whose bbox leaves the text block
W, H = doc[0].rect.width, doc[0].rect.height
margin = 2.4 / 2.54 * 72  # geometry package: 2.4 cm
over = []
for pno, page in enumerate(doc, start=1):
    for block in page.get_text("dict")["blocks"]:
        for line in block.get("lines", []):
            for span in line.get("spans", []):
                x0, y0, x1, y1 = span["bbox"]
                if x1 > W - margin + 1.5 or x0 < margin - 1.5:
                    over.append((pno, round(x1, 1), span["text"][:40]))
print("spans outside the text block:", len(over))
for row in over[:8]:
    print("   ", row)

# 3. every number macro in numbers.tex should have its value somewhere in the text
macros = {}
src = pathlib.Path("tex/numbers.tex").read_text(encoding="utf-8")
for m in re.finditer(r"\\csname pv@([^\}]+)\\endcsname\{([^}]*)\}", src):
    macros[m.group(1)] = m.group(2)

used = []
for m in re.finditer(r"\\(?:val|valsd|valn|valfrac|valtrue|valuniq)\{([^}]+)\}", pathlib.Path("tex/main.tex").read_text(encoding="utf-8")):
    used.append(m.group(1))
keymap = {"val": "pv@", "valsd": "pvsd@", "valn": "pvn@", "valfrac": "pvfrac@",
          "valtrue": "pvtrue@", "valuniq": "pvuniq@"}
used_pairs = []
for m in re.finditer(r"\\(val|valsd|valn|valfrac|valtrue|valuniq)\{([^}]+)\}", pathlib.Path("tex/main.tex").read_text(encoding="utf-8")):
    used_pairs.append((m.group(1), m.group(2)))

missing = []
for kind, key in sorted(set(used_pairs)):
    if kind != "val":
        continue
    value = macros.get(key)
    if value is None:
        missing.append((key, "no macro"))
        continue
    if value.strip() not in text:
        missing.append((key, value))
print("lookups whose value is not found in the PDF text:", len(missing))
for row in missing[:12]:
    print("   ", row)

print("pages:", doc.page_count)
