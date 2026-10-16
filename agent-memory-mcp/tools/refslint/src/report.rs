//! `refslint report` -- validate a protocol report against code/protocol_schema.json.
//!
//! Implements the same subset of JSON Schema (draft 2020-12) the Python validator
//! implemented: `$ref` to `$defs`, `type`, `required`, `properties`,
//! `additionalProperties` (false or a schema), `items`, `minItems`, `enum`, `pattern`,
//! `minimum`, `maximum`, `propertyNames`. Keywords outside that subset (e.g. `minLength`)
//! are ignored here exactly as they were ignored there - see TOOLING.md.

use crate::json::{self, Json};

fn resolve<'a>(node: &'a Json, schema: &'a Json) -> Result<&'a Json, String> {
    let mut current = node;
    while let Some(reference) = current.get("$ref").and_then(|r| r.as_str()) {
        if !reference.starts_with("#/") {
            return Err(format!("external $ref not supported: {reference}"));
        }
        let mut target = schema;
        for part in reference[2..].split('/') {
            target = target
                .get(part)
                .ok_or_else(|| format!("$ref {reference} does not resolve"))?;
        }
        current = target;
    }
    Ok(current)
}

fn type_matches(name: &str, node: &Json) -> bool {
    match name {
        "object" => matches!(node, Json::Obj(_)),
        "array" => matches!(node, Json::Arr(_)),
        "string" => matches!(node, Json::Str(_)),
        "integer" => matches!(node, Json::Int(_)),
        "number" => node.is_number() && !node.is_bool(),
        "boolean" => matches!(node, Json::Bool(_)),
        "null" => node.is_null(),
        _ => false,
    }
}

fn validate(node: &Json, sch: &Json, schema: &Json, path: &str, errors: &mut Vec<String>) -> Result<(), String> {
    let sch = resolve(sch, schema)?;

    if let Some(expected) = sch.get("type") {
        let names: Vec<String> = match expected {
            Json::Arr(items) => items
                .iter()
                .filter_map(|i| i.as_str().map(|s| s.to_string()))
                .collect(),
            other => vec![json::py_str(other)],
        };
        if !names.iter().any(|n| type_matches(n, node)) {
            errors.push(format!(
                "{}: expected {}, got {}",
                if path.is_empty() { "<root>" } else { path },
                json::py_str(expected),
                node.type_name()
            ));
            return Ok(());
        }
    }

    if let Some(values) = sch.get("enum") {
        if let Json::Arr(items) = values {
            if !items.iter().any(|i| i == node) {
                errors.push(format!(
                    "{}: {} not in enum {}",
                    path,
                    json::py_repr(node),
                    json::py_repr(values)
                ));
            }
        }
    }

    if let Some(pattern) = sch.get("pattern").and_then(|p| p.as_str()) {
        if let Json::Str(text) = node {
            if !crate::regex::is_match(pattern, text)? {
                errors.push(format!(
                    "{}: {} does not match pattern {}",
                    path,
                    json::py_repr(node),
                    pattern
                ));
            }
        }
    }

    if node.is_number() {
        if let Some(minimum) = sch.get("minimum") {
            if let (Some(value), Some(min)) = (node.as_f64(), minimum.as_f64()) {
                if value < min {
                    errors.push(format!(
                        "{}: {} < minimum {}",
                        path,
                        json::py_str(node),
                        json::py_str(minimum)
                    ));
                }
            }
        }
        if let Some(maximum) = sch.get("maximum") {
            if let (Some(value), Some(max)) = (node.as_f64(), maximum.as_f64()) {
                if value > max {
                    errors.push(format!(
                        "{}: {} > maximum {}",
                        path,
                        json::py_str(node),
                        json::py_str(maximum)
                    ));
                }
            }
        }
    }

    if let Json::Obj(members) = node {
        let root_label = if path.is_empty() { "<root>".to_string() } else { path.to_string() };
        if let Some(required) = sch.get("required").and_then(|r| r.as_arr()) {
            for req in required {
                if let Some(name) = req.as_str() {
                    if node.get(name).is_none() {
                        errors.push(format!("{root_label}: missing required field '{name}'"));
                    }
                }
            }
        }
        let props = sch.get("properties");
        if sch.get("additionalProperties") == Some(&Json::Bool(false)) {
            for (key, _) in members {
                let known = props.map(|p| p.get(key).is_some()).unwrap_or(false);
                if !known {
                    errors.push(format!("{root_label}: unexpected field '{key}'"));
                }
            }
        }
        if let Some(pn) = sch.get("propertyNames") {
            let pn = resolve(pn, schema)?;
            if let Some(allowed) = pn.get("enum").and_then(|e| e.as_arr()) {
                for (key, _) in members {
                    if !allowed.iter().any(|a| a.as_str() == Some(key.as_str())) {
                        errors.push(format!("{root_label}: property name '{key}' not in enum"));
                    }
                }
            }
        }
        if let Some(props) = props {
            for (key, sub) in props.as_obj().into_iter().flatten() {
                if let Some(value) = node.get(key) {
                    validate(
                        value,
                        sub,
                        schema,
                        &format!("{path}.{key}"),
                        errors,
                    )?;
                }
            }
        }
        if let Some(ap) = sch.get("additionalProperties") {
            if matches!(ap, Json::Obj(_)) {
                for (key, value) in members {
                    let known = props.map(|p| p.get(key).is_some()).unwrap_or(false);
                    if !known {
                        validate(value, ap, schema, &format!("{path}.{key}"), errors)?;
                    }
                }
            }
        }
    }

    if let Json::Arr(items) = node {
        if let Some(min_items) = sch.get("minItems").and_then(|m| m.as_f64()) {
            if (items.len() as f64) < min_items {
                errors.push(format!(
                    "{}: {} items < minItems {}",
                    path,
                    items.len(),
                    min_items as i64
                ));
            }
        }
        if let Some(item_schema) = sch.get("items") {
            for (i, item) in items.iter().enumerate() {
                validate(item, item_schema, schema, &format!("{path}[{i}]"), errors)?;
            }
        }
    }

    Ok(())
}

fn is_none_field(rec: &Json, key: &str) -> bool {
    // Python's `x.get(key) is None`: missing key or a JSON null.
    matches!(rec.get(key), None | Some(Json::Null))
}

fn check_semantics(rep: &Json, errors: &mut Vec<String>) {
    let families: Vec<&Json> = rep
        .get("families")
        .and_then(|f| f.as_arr())
        .map(|f| f.iter().collect())
        .unwrap_or_default();
    let mut family_ids: Vec<String> = Vec::new();
    for f in &families {
        let id = f.get("id").and_then(|i| i.as_str()).unwrap_or("").to_string();
        family_ids.push(id.clone());
        let skipped = matches!(f.get("skipped"), Some(Json::Bool(true)));
        let has_skip_reason = match f.get("skip_reason") {
            Some(Json::Str(s)) => !s.is_empty(),
            Some(Json::Null) | None => false,
            Some(Json::Bool(b)) => *b,
            Some(_) => true,
        };
        if skipped && !has_skip_reason {
            errors.push(format!("semantics: family {id} skipped without skip_reason"));
        }
        let metrics: Vec<&Json> = f
            .get("metrics")
            .and_then(|m| m.as_arr())
            .map(|m| m.iter().collect())
            .unwrap_or_default();
        for m in metrics {
            let name = m.get("name").and_then(|n| n.as_str()).unwrap_or("");
            let is_graded = m.get("computer").and_then(|c| c.as_str()) == Some("rubric_two_graders");
            let has_value = !is_none_field(m, "value");
            if is_graded && has_value && is_none_field(m, "kappa") {
                errors.push(format!(
                    "semantics: family {id} metric '{name}' reports a graded value but no kappa"
                ));
            }
            let ci_method_none = rep
                .get("statistics")
                .and_then(|s| s.get("ci_method"))
                .and_then(|c| c.as_str())
                == Some("none");
            if has_value && (is_none_field(m, "ci_low") || is_none_field(m, "ci_high")) && !ci_method_none
            {
                errors.push(format!(
                    "semantics: family {id} metric '{name}' has a value but no CI interval"
                ));
            }
            if let (Some(low), Some(high)) = (
                m.get("ci_low").and_then(|v| v.as_f64()),
                m.get("ci_high").and_then(|v| v.as_f64()),
            ) {
                if low > high {
                    errors.push(format!("semantics: family {id} metric '{name}' has ci_low > ci_high"));
                }
            }
        }
    }
    if let Some(ops) = rep.get("operations").and_then(|o| o.as_obj()) {
        for (name, block) in ops {
            let supported = block.get("supported").and_then(|s| s.as_str()).unwrap_or("");
            let has_metrics = block
                .get("metrics")
                .and_then(|m| m.as_arr())
                .map(|m| !m.is_empty())
                .unwrap_or(false);
            if (supported == "yes" || supported == "partial") && !has_metrics {
                errors.push(format!(
                    "semantics: operation {name} claims support but reports no metrics"
                ));
            }
        }
    }
    if !family_ids.iter().any(|id| id == "T10") {
        errors.push("semantics: adversarial family T10 absent without an explicit skip entry".to_string());
    }
}

/// Validate one already-parsed report. Returns the error list (empty = valid).
pub fn validate_report(rep: &Json, schema: &Json) -> Result<Vec<String>, String> {
    let mut errors = Vec::new();
    validate(rep, schema, schema, "", &mut errors)?;
    if errors.is_empty() {
        check_semantics(rep, &mut errors);
    }
    Ok(errors)
}

pub fn run(schema_path: &str, reports: &[String]) -> i32 {
    let schema = match json::parse_file(schema_path) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("refslint report: cannot read {schema_path}: {e}");
            return 2;
        }
    };
    if reports.is_empty() {
        eprintln!("usage: refslint report [--schema code/protocol_schema.json] <report.json> ...");
        return 2;
    }
    let mut bad = 0;
    for path in reports {
        let rep = match json::parse_file(path) {
            Ok(r) => r,
            Err(e) => {
                println!("INVALID {path}: cannot parse JSON: {e}");
                bad += 1;
                continue;
            }
        };
        let errors = match validate_report(&rep, &schema) {
            Ok(e) => e,
            Err(e) => {
                println!("INVALID {path}: {e}");
                bad += 1;
                continue;
            }
        };
        if errors.is_empty() {
            let families = rep.get("families").and_then(|f| f.as_arr()).map(|f| f.len()).unwrap_or(0);
            let metrics: usize = rep
                .get("families")
                .and_then(|f| f.as_arr())
                .map(|f| {
                    f.iter()
                        .map(|fam| fam.get("metrics").and_then(|m| m.as_arr()).map(|m| m.len()).unwrap_or(0))
                        .sum()
                })
                .unwrap_or(0);
            let subject = rep
                .get("subject")
                .and_then(|s| s.get("name"))
                .and_then(|n| n.as_str())
                .unwrap_or("");
            println!(
                "VALID   {path}  (subject={subject}, families={families}, metric entries={metrics})"
            );
        } else {
            bad += 1;
            println!("INVALID {path} ({} problems):", errors.len());
            for e in errors.iter().take(25) {
                println!("   - {e}");
            }
            if errors.len() > 25 {
                println!("   ... {} more", errors.len() - 25);
            }
        }
    }
    if bad > 0 {
        1
    } else {
        0
    }
}
