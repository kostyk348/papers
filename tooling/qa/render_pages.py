"""QA-only: render PDF pages to PNG so a human (or an agent with vision) can look at the
layout. This is not part of the paper pipeline; the pipeline is Rust plus tectonic."""
import sys, pathlib
import fitz

pdf = sys.argv[1]
outdir = pathlib.Path(sys.argv[2])
outdir.mkdir(parents=True, exist_ok=True)
doc = fitz.open(pdf)
print("pages:", doc.page_count)
for i, page in enumerate(doc):
    pix = page.get_pixmap(dpi=110)
    path = outdir / f"page{i+1:02d}.png"
    pix.save(path)
    print(path)
