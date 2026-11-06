//! `markshot schema` prints the JSON schema that agents write specs against.

use std::collections::BTreeSet;

use assert_cmd::cargo::cargo_bin_cmd;
use serde_json::Value;

fn schema() -> Value {
    let output = cargo_bin_cmd!("markshot")
        .arg("schema")
        .output()
        .expect("markshot runs");
    assert!(output.status.success(), "{output:?}");
    serde_json::from_slice(&output.stdout).expect("stdout is JSON")
}

/// Collects the `const` of every object schema's `type` property: the annotation type names.
fn type_names(value: &Value, names: &mut BTreeSet<String>) {
    match value {
        Value::Object(map) => {
            if let Some(name) = map
                .get("properties")
                .and_then(|properties| properties.get("type"))
                .and_then(|tag| tag.get("const"))
                .and_then(Value::as_str)
            {
                names.insert(name.to_owned());
            }
            for child in map.values() {
                type_names(child, names);
            }
        }
        Value::Array(items) => {
            for item in items {
                type_names(item, names);
            }
        }
        _ => {}
    }
}

#[test]
fn schema_is_json_schema() {
    let schema = schema();
    assert!(schema.get("$schema").is_some(), "{schema:#}");
    assert!(
        schema["properties"].get("annotations").is_some(),
        "{schema:#}"
    );
}

#[test]
fn schema_lists_every_annotation_type() {
    let mut names = BTreeSet::new();
    type_names(&schema(), &mut names);
    let want = [
        "arrow",
        "blur",
        "counter",
        "highlight",
        "line",
        "oval",
        "rect",
        "spotlight",
    ];
    assert_eq!(names, BTreeSet::from(want.map(String::from)));
}
