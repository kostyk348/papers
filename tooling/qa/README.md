# QA scaffolding (not part of any paper pipeline)

These two scripts are the only Python in `papers/tooling`, and they are here rather than in
a paper package for one reason: checking a built PDF needs a mature PDF reader, and there is
no dependency-free way to rasterise a page or read a text block's bounding box from Rust in
reasonable time. The house rule (service tooling in Rust, Python only where a foreign mature
library cannot be replaced) therefore applies to PDF *inspection* and to nothing else.

They are report-only. No number in any paper comes from them, and `verify_all.sh` does not
call them.

    python tooling/qa/qa_pdf.py path/to/main.pdf      # undefined macros, margin overflow,
                                                      # and whether every \val value reaches the page
    python tooling/qa/render_pages.py in.pdf outdir   # page PNGs for a visual look

`qa_pdf.py` expects to be run from the paper folder (it reads `tex/numbers.tex` and
`tex/main.tex` relative to the working directory).
