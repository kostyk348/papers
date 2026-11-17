//! papertool -- one utility, one binary, subcommands, standard library only.
//!
//! The pipeline shared by every paper package in this repository:
//!
//!   * the curated source list -> verified `references.bib` (+ a verification table) and
//!     citation-key checks, with network verification of every DOI / arXiv id / URL;
//!   * the experiment plan -> raw logs, peak memory, aggregated `data/measurements.json`;
//!   * `data/measurements.json` -> the number macros, the number ledger and the figure CSVs;
//!   * the checks that refuse a paper whose prose contains a number that no measurement
//!     produced.
//!
//! Usage: `papertool <subcommand> [options]`, `papertool help`.

mod bib;
mod cites;
mod derive;
mod json;
mod ledger;
mod measure;
mod merge;
mod net;
mod numbers;
mod plan;
mod spec;
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
papertool -- pipeline utility for the paper packages in papers/.

Papers and citations
  bib         [--root .] [--refs data/sources.toml] [--meta data/refmeta.json]
              [--out-dir .] [--check-only]      -> tex/references.bib,
                                                   reference-verification.md,
                                                   data/refs-verified.csv
  cites       [--root .] [--refs data/sources.toml] [--check | --count]
  jsoncheck   <file.json> ...                   parse-only check of JSON artefacts

Experiments and numbers
  measure     [--root .] [--plan experiments/plan.toml] [--out data/measurements.json]
              [--logs evidence/raw] [--only <run|group>] [--check] [--derive-only]
                                                run the plan, keep raw logs, aggregate
  ledger      [--root .] [--measurements data/measurements.json]
              [--numbers-tex tex/numbers.tex] [--ledger-tsv data/numbers.tsv]
              [--figures-dir data/figures] [--check]
                                                -> number macros, TSV ledger, figure CSVs
  numbers     [--root .] [--numbers-tex tex/numbers.tex]
              [--allowlist data/numeral-allowlist.tsv] [--texdir tex]
                                                every number macro resolves; no unlisted
                                                bare numerals in the prose

Network verification (HTTPS via curl)
  fetch       --queries queries.json --out evidence/raw/arxiv1.json [--max 8]
  ids         --ids ids.txt   --out evidence/raw/arxiv_ids.json
  doi         --dois dois.txt --out evidence/raw/dois.json
  probe       --urls urls.txt --out evidence/raw/urls.json
  query       --queries queries.txt --out evidence/raw/crossref.json
  merge       [--raw evidence/raw] [--out data/refmeta.json]"
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
        "bib" => &["root", "out-dir", "refs", "meta"],
        "cites" => &["root", "refs"],
        "merge" => &["raw", "out"],
        "jsoncheck" => &[],
        "measure" => &["root", "plan", "out", "logs", "only"],
        "ledger" => &[
            "root",
            "measurements",
            "numbers-tex",
            "ledger-tsv",
            "figures-dir",
        ],
        "numbers" => &["root", "numbers-tex", "allowlist", "texdir"],
        "fetch" => &["queries", "out", "max"],
        "ids" => &["ids", "out"],
        "doi" => &["dois", "out"],
        "probe" => &["urls", "out"],
        "query" => &["queries", "out"],
        other => {
            eprintln!("papertool: unknown subcommand '{other}'");
            usage();
            std::process::exit(2);
        }
    };
    let opts = match parse(rest, valued) {
        Ok(o) => o,
        Err(e) => {
            eprintln!("papertool {sub}: {e}");
            std::process::exit(2);
        }
    };

    let code = match sub.as_str() {
        "bib" => {
            let root = opts.get("root", &opts.get("out-dir", "."));
            bib::run(&bib::Options {
                root: &root,
                spec_path: &opts.get("refs", "data/sources.toml"),
                meta_path: &opts.get("meta", "data/refmeta.json"),
                check_only: opts.flag("check-only"),
                legacy_banner: false,
            })
        }
        "cites" => {
            let root = opts.get("root", ".");
            let refs = opts.get("refs", "data/sources.toml");
            if opts.flag("count") {
                cites::count(&root)
            } else {
                cites::check(&root, &refs)
            }
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
        "measure" => measure::run(&measure::Options {
            root: &opts.get("root", "."),
            plan: &opts.get("plan", "experiments/plan.toml"),
            out: &opts.get("out", "data/measurements.json"),
            logs: &opts.get("logs", "evidence/raw"),
            only: opts.get_opt("only"),
            check: opts.flag("check"),
            derive_only: opts.flag("derive-only"),
        }),
        "ledger" => ledger::run(&ledger::Options {
            root: &opts.get("root", "."),
            measurements: &opts.get("measurements", "data/measurements.json"),
            numbers_tex: &opts.get("numbers-tex", "tex/numbers.tex"),
            ledger_tsv: &opts.get("ledger-tsv", "data/numbers.tsv"),
            figures_dir: &opts.get("figures-dir", "data/figures"),
            check: opts.flag("check"),
        }),
        "numbers" => numbers::run(&numbers::Options {
            root: &opts.get("root", "."),
            numbers_tex: &opts.get("numbers-tex", "tex/numbers.tex"),
            allowlist: &opts.get("allowlist", "data/numeral-allowlist.tsv"),
            texdir: &opts.get("texdir", "tex"),
        }),
        "fetch" => net::fetch(
            &opts.get("queries", "queries.json"),
            &opts.get("out", "evidence/raw/arxiv.json"),
            opts.get("max", "8").parse().unwrap_or(8),
        ),
        "ids" => net::ids(
            &opts.get("ids", "ids.txt"),
            &opts.get("out", "evidence/raw/arxiv_ids.json"),
        ),
        "doi" => net::doi(
            &opts.get("dois", "dois.txt"),
            &opts.get("out", "evidence/raw/dois.json"),
        ),
        "probe" => net::probe(
            &opts.get("urls", "urls.txt"),
            &opts.get("out", "evidence/raw/urls.json"),
        ),
        "query" => net::query(
            &opts.get("queries", "queries.txt"),
            &opts.get("out", "evidence/raw/crossref.json"),
        ),
        _ => 2,
    };
    std::process::exit(code);
}
