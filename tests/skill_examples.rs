//! The agent skill's examples: every `json` fenced block in `skills/annotating-screenshots/SKILL.md` is a
//! complete spec that renders on an 800×600 screenshot without warnings, and together they
//! show every tool.

mod common;

use std::fs;
use std::path::PathBuf;

use markshot::render::render;
use markshot::spec::{Annotation, Spec};

/// The bodies of the `json` fenced blocks of `markdown`, in order.
fn json_blocks(markdown: &str) -> Vec<String> {
    let mut blocks = Vec::new();
    let mut current: Option<String> = None;
    for line in markdown.lines() {
        match (&mut current, line.trim_end()) {
            (None, "```json") => current = Some(String::new()),
            (Some(block), "```") => {
                blocks.push(std::mem::take(block));
                current = None;
            }
            (Some(block), _) => {
                block.push_str(line);
                block.push('\n');
            }
            (None, _) => {}
        }
    }
    assert!(current.is_none(), "unclosed ```json block");
    blocks
}

#[test]
fn skill_json_examples_render() {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("skills/annotating-screenshots/SKILL.md");
    let skill = fs::read_to_string(&path).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
    let blocks = json_blocks(&skill);
    let mut kinds = Vec::new();
    for (i, block) in blocks.iter().enumerate() {
        let spec = Spec::from_json(block).unwrap_or_else(|e| panic!("block {i}: {e}\n{block}"));
        let (_, report) = render(common::fixture(800, 600), &spec, None)
            .unwrap_or_else(|e| panic!("block {i}: {e}\n{block}"));
        assert!(
            report.warnings.is_empty(),
            "block {i}: {:?}\n{block}",
            report.warnings
        );
        kinds.extend(spec.annotations.iter().map(|a| match a {
            Annotation::Arrow(_) => "arrow",
            Annotation::Rect(_) => "rect",
            Annotation::Oval(_) => "oval",
            Annotation::Blur(_) => "blur",
            Annotation::Highlight(_) => "highlight",
            Annotation::Spotlight(_) => "spotlight",
            Annotation::Counter(_) => "counter",
            Annotation::Line(_) => "line",
        }));
    }
    for kind in [
        "arrow",
        "rect",
        "oval",
        "blur",
        "highlight",
        "spotlight",
        "counter",
        "line",
    ] {
        assert!(kinds.contains(&kind), "no example uses `{kind}`");
    }
}
