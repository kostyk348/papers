//! `refslint selftest` -- regenerate the deliberately broken protocol reports and assert
//! that the validator rejects every one of them (the negative tests `verify_all.sh` runs).

use crate::json::{self, Json};
use crate::report;

pub fn run(root: &str, schema_path: &str) -> i32 {
    let example_path = format!("{}/data/example-report.json", root.trim_end_matches('/'));
    let out_dir = format!("{}/data/bad-reports", root.trim_end_matches('/'));
    let base = match json::parse_file(&example_path) {
        Ok(b) => b,
        Err(e) => {
            eprintln!("refslint selftest: cannot read {example_path}: {e}");
            return 2;
        }
    };

    let mut cases: Vec<(String, Json)> = Vec::new();

    // 1. missing disclosure block
    let mut r = base.clone();
    r.remove("disclosure");
    cases.push(("bad-missing-disclosure.json".to_string(), r));

    // 2. metric with a value but no CI, and no kappa on a two-grader metric
    let mut r = base.clone();
    if let Some(value) = r
        .get_mut("families")
        .and_then(|f| f.as_arr_mut())
        .and_then(|f| f.get_mut(1))
        .and_then(|fam| fam.get_mut("metrics"))
        .and_then(|m| m.as_arr_mut())
        .and_then(|m| m.get_mut(0))
        .and_then(|metric| metric.get_mut("value"))
    {
        *value = Json::Float(0.42);
    }
    if let Some(value) = r
        .get_mut("families")
        .and_then(|f| f.as_arr_mut())
        .and_then(|f| f.get_mut(5))
        .and_then(|fam| fam.get_mut("metrics"))
        .and_then(|m| m.as_arr_mut())
        .and_then(|m| m.get_mut(3))
        .and_then(|metric| metric.get_mut("value"))
    {
        *value = Json::Float(4.1);
    }
    cases.push(("bad-metric-integrity.json".to_string(), r));

    // 3. operation claims support without any metrics
    let mut r = base.clone();
    let mut block = Json::Obj(Vec::new());
    block.set("supported", Json::Str("yes".to_string()));
    if let Some(ops) = r.get_mut("operations") {
        ops.set("O4_REVISE", block);
    }
    cases.push(("bad-operation-claim.json".to_string(), r));

    // 4. subject with a version string instead of a commit sha
    let mut r = base.clone();
    if let Some(subject) = r.get_mut("subject") {
        subject.set("commit_sha", Json::Str("v1.2.3".to_string()));
    }
    cases.push(("bad-commit-sha.json".to_string(), r));

    // 5. T10 silently removed
    let mut r = base.clone();
    if let Some(families) = r.get_mut("families").and_then(|f| f.as_arr_mut()) {
        families.retain(|f| f.get("id").and_then(|i| i.as_str()) != Some("T10"));
    }
    cases.push(("bad-t10-removed.json".to_string(), r));

    let schema = match json::parse_file(schema_path) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("refslint selftest: cannot read {schema_path}: {e}");
            return 2;
        }
    };

    let mut failed = 0;
    for (name, payload) in &cases {
        let path = format!("{out_dir}/{name}");
        let text = json::dump(payload, 1, false);
        if let Err(e) = crate::util::write_text_py(&path, &text) {
            eprintln!("refslint selftest: {e}");
            return 2;
        }
        println!("wrote {path}");
        match report::validate_report(payload, &schema) {
            Ok(errors) if errors.is_empty() => {
                println!("REGRESSION: {name} was accepted by the validator");
                failed += 1;
            }
            Ok(errors) => {
                println!("rejected {name} ({} problems): {}", errors.len(), errors[0]);
            }
            Err(e) => {
                println!("REGRESSION: {name}: validator error: {e}");
                failed += 1;
            }
        }
    }

    println!(
        "selftest: {} of {} broken reports rejected, {} accepted",
        cases.len() - failed,
        cases.len(),
        failed
    );
    if failed == 0 {
        0
    } else {
        1
    }
}
