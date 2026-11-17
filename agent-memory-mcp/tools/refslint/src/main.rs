//! refslint -- one utility, one binary, subcommands, standard library only.
//!
//! Everything the agent-memory-mcp paper package needs to stay honest about its own
//! numbers and citations: the curated source spec, the bibliography and verification
//! table, citation-key checks, protocol-report validation, the negative self-tests,
//! reference-implementation facts, the metadata snapshot merge, and the network
//! verification commands (arXiv, Crossref, HTTP probes).
//!
//! Usage: `refslint <subcommand> [options]`, `refslint help`.

mod bib;
mod cites;
mod facts;
mod json;
mod merge;
mod net;
mod regex;
mod report;
mod selftest;
mod spec;
mod tables;
mod util;

struct Opts {
    values: Vec<(String, String)>,
    flags: Vec<String>,
    positional: Vec<String>,
}

impl Opts {
    fn get(&self, name: &str, default: &str) -> String {
        self.values
            .iter()
            .rev()
            .find(|(k, _)| k == name)
            .map(|(_, v)| v.clone())
            .unwrap_or_else(|| default.to_string())
    }

    fn get_opt(&self, name: &str) -> Option<String> {
        self.values
            .iter()
            .rev()
            .find(|(k, _)| k == name)
            .map(|(_, v)| v.clone())
    }

    fn flag(&self, name: &str) -> bool {
        self.flags.iter().any(|f| f == name)
    }
}

fn parse(args: &[String], valued: &[&str]) -> Result<Opts, String> {
    let mut opts = Opts {
        values: Vec::new(),
        flags: Vec::new(),
        positional: Vec::new(),
    };
    let mut i = 0usize;
    while i < args.len() {
        let arg = &args[i];
        if let Some(rest) = arg.strip_prefix("--") {
            if let Some((name, value)) = rest.split_once('=') {
                opts.values.push((name.to_string(), value.to_string()));
                i += 1;
                continue;
            }
            if valued.contains(&rest) {
                let value = args
                    .get(i + 1)
                    .ok_or_else(|| format!("--{rest} needs a value"))?;
                opts.values.push((rest.to_string(), value.clone()));
                i += 2;
                continue;
            }
            opts.flags.push(rest.to_string());
            i += 1;
            continue;
        }
        opts.positional.push(arg.clone());
        i += 1;
    }
    Ok(opts)
}

fn usage() {
    println!(
        "\
refslint -- pipeline utility for the agent-memory-mcp paper package.

Curated sources and artefacts
  import-spec --py code/refs.py --out code/refs.toml   convert the old Python spec once
  bib         [--root .] [--refs code/refs.toml] [--meta data/refmeta.json]
              [--out-dir .] [--check-only] [--legacy-banner]
                                                       -> tex/references.bib,
                                                          reference-verification.md,
                                                          data/refs-verified.csv
  cites       [--root .] [--refs code/refs.toml] [--check | --count]
  jsoncheck   <file.json> ...                          parse-only check of JSON artefacts

Protocol reports
  report      [--schema code/protocol_schema.json] <report.json> ...
  selftest    [--root .] [--schema code/protocol_schema.json]
                                                       regenerate the 5 broken reports and
                                                       assert all of them are rejected
  tables      [--root .] [--check] [<report.json> ...]
                                                       -> tex/tables/*.tex, tex/numbers.tex,
                                                          data/run-numbers.tsv.  Every table
                                                          and every inline number of the
                                                          paper comes from the reports; --check
                                                          fails on a stale file or an unknown
                                                          \\val{{...}} lookup in tex/main.tex

Reference implementation and metadata
  facts       --refs-dir <dir with the clones> --out data/repo-facts.json
  merge       --raw evidence/raw --out data/refmeta.json
  abs         --store data/refmeta.json --ids ids.txt [--n 420]   abstract dumps

Network verification (HTTPS via curl)
  fetch       --queries queries.json --out evidence/raw/arxiv1.json [--max 8]
  ids         --ids ids.txt   --out evidence/raw/arxiv_ids.json
  doi         --dois dois.txt --out evidence/raw/dois.json
  probe       --urls urls.txt --out evidence/raw/urls.json
  query       --queries queries.txt --out evidence/raw/crossref.json"
    );
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.is_empty() || args[0] == "help" || args[0] == "--help" || args[0] == "-h" {
        usage();
        std::process::exit(if args.is_empty() { 2 } else { 0 });
    }
    let sub = args[0].clone();
    let rest = &args[1..];
    let valued: &[&str] = match sub.as_str() {
        "import-spec" => &["py", "out"],
        "bib" => &["root", "out-dir", "refs", "meta"],
        "cites" => &["root", "refs"],
        "report" => &["schema"],
        "selftest" => &["root", "schema"],
        "tables" => &["root"],
        "facts" => &["refs-dir", "out"],
        "merge" => &["raw", "out"],
        "jsoncheck" => &[],
        "abs" => &["store", "ids", "n"],
        "fetch" => &["queries", "out", "max"],
        "ids" => &["ids", "out"],
        "doi" => &["dois", "out"],
        "probe" => &["urls", "out"],
        "query" => &["queries", "out"],
        other => {
            eprintln!("refslint: unknown subcommand '{other}'");
            usage();
            std::process::exit(2);
        }
    };
    let opts = match parse(rest, valued) {
        Ok(o) => o,
        Err(e) => {
            eprintln!("refslint {sub}: {e}");
            std::process::exit(2);
        }
    };

    let code = match sub.as_str() {
        "import-spec" => {
            let py = opts.get("py", "code/refs.py");
            let out = opts.get("out", "code/refs.toml");
            match spec::import_python_spec(&py).and_then(|sources| {
                spec::write_toml(&out, &sources).map(|()| sources.len())
            }) {
                Ok(n) => {
                    println!("converted {n} curated sources: {py} -> {out}");
                    0
                }
                Err(e) => {
                    eprintln!("refslint import-spec: {e}");
                    2
                }
            }
        }
        "bib" => {
            let root = opts.get("root", &opts.get("out-dir", "."));
            bib::run(&bib::Options {
                root: &root,
                spec_path: &opts.get("refs", "code/refs.toml"),
                meta_path: &opts.get("meta", "data/refmeta.json"),
                check_only: opts.flag("check-only"),
                legacy_banner: opts.flag("legacy-banner"),
            })
        }
        "cites" => {
            let root = opts.get("root", ".");
            let refs = opts.get("refs", "code/refs.toml");
            if opts.flag("count") {
                cites::count(&root)
            } else {
                cites::check(&root, &refs)
            }
        }
        "report" => report::run(&opts.get("schema", "code/protocol_schema.json"), &opts.positional),
        "selftest" => selftest::run(
            &opts.get("root", "."),
            &opts.get("schema", "code/protocol_schema.json"),
        ),
        "tables" => tables::run(&tables::Options {
            root: &opts.get("root", "."),
            reports: &opts.positional,
            check: opts.flag("check"),
        }),
        "facts" => {
            let refs_dir = match opts.get_opt("refs-dir") {
                Some(d) => d,
                None => {
                    eprintln!("refslint facts: --refs-dir is required");
                    std::process::exit(2);
                }
            };
            facts::run(&refs_dir, &opts.get("out", "data/repo-facts.json"))
        }
        "merge" => merge::run(
            &opts.get("raw", "evidence/raw"),
            &opts.get("out", "data/refmeta.json"),
        ),
        "jsoncheck" => {
            let mut bad = 0;
            for path in &opts.positional {
                if let Err(e) = json::parse_file(path) {
                    eprintln!("{path}: {e}");
                    bad += 1;
                }
            }
            if bad > 0 {
                println!("{bad} JSON file(s) failed to parse");
                1
            } else {
                println!("all {} JSON files parse", opts.positional.len());
                0
            }
        }
        "abs" => net::abs_dump(
            &opts.get("store", "data/refmeta.json"),
            &opts.get("ids", "ids.txt"),
            opts.get("n", "420").parse().unwrap_or(420),
        ),
        "fetch" => net::fetch(
            &opts.get("queries", "queries.json"),
            &opts.get("out", "evidence/raw/arxiv.json"),
            opts.get("max", "8").parse().unwrap_or(8),
        ),
        "ids" => net::ids(&opts.get("ids", "ids.txt"), &opts.get("out", "evidence/raw/arxiv_ids.json")),
        "doi" => net::doi(&opts.get("dois", "dois.txt"), &opts.get("out", "evidence/raw/dois.json")),
        "probe" => net::probe(&opts.get("urls", "urls.txt"), &opts.get("out", "evidence/raw/urls.json")),
        "query" => net::query(
            &opts.get("queries", "queries.txt"),
            &opts.get("out", "evidence/raw/crossref.json"),
        ),
        _ => 2,
    };
    std::process::exit(code);
}
