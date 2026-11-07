//! The agent skill in `skills/annotating-screenshots/`:
//! - its `SKILL.md` frontmatter follows the Agent Skills specification (agentskills.io), and
//! - every `json` fenced block in `SKILL.md` or `references/*.md` is a complete spec that renders
//!   on an 800×600 screenshot without warnings; together they show every tool.

mod common;

use std::fs;
use std::path::{Path, PathBuf};

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

fn skill_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("skills/annotating-screenshots")
}

fn read(path: &Path) -> String {
    fs::read_to_string(path).unwrap_or_else(|e| panic!("{}: {e}", path.display()))
}

/// `SKILL.md` and every markdown file under `references/`, sorted by path.
fn skill_markdown() -> Vec<(PathBuf, String)> {
    let dir = skill_dir();
    let mut paths = vec![dir.join("SKILL.md")];
    let mut refs: Vec<PathBuf> = fs::read_dir(dir.join("references"))
        .expect("references/ exists")
        .map(|e| e.expect("dir entry").path())
        .filter(|p| p.extension().is_some_and(|e| e == "md"))
        .collect();
    refs.sort();
    paths.extend(refs);
    paths
        .into_iter()
        .map(|p| {
            let text = read(&p);
            (p, text)
        })
        .collect()
}

/// The `key: value` lines of the YAML frontmatter, top-level keys only.
fn frontmatter(skill: &str) -> Vec<(String, String)> {
    let body = skill
        .strip_prefix("---\n")
        .expect("SKILL.md starts with ---");
    let end = body.find("\n---\n").expect("frontmatter is closed by ---");
    body[..end]
        .lines()
        .filter(|l| !l.starts_with(' ') && !l.is_empty())
        .map(|l| {
            let (k, v) = l
                .split_once(':')
                .unwrap_or_else(|| panic!("not `key: value`: {l}"));
            (k.to_owned(), v.trim().to_owned())
        })
        .collect()
}

#[test]
fn skill_frontmatter_follows_the_agent_skills_spec() {
    let fields = frontmatter(&read(&skill_dir().join("SKILL.md")));
    let get = |k: &str| {
        fields
            .iter()
            .find(|(key, _)| key == k)
            .map(|(_, v)| v.as_str())
    };
    for (key, _) in &fields {
        assert!(
            [
                "name",
                "description",
                "license",
                "compatibility",
                "metadata",
                "allowed-tools"
            ]
            .contains(&key.as_str()),
            "unknown frontmatter field `{key}`"
        );
    }
    let name = get("name").expect("name");
    assert!((1..=64).contains(&name.len()), "name length");
    assert!(
        name.chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-')
            && !name.starts_with('-')
            && !name.ends_with('-')
            && !name.contains("--"),
        "name `{name}`: lowercase letters, digits and single inner hyphens only"
    );
    assert_eq!(
        Some(name),
        skill_dir().file_name().and_then(|n| n.to_str()),
        "name matches the directory"
    );
    let description = get("description").expect("description");
    assert!(
        (1..=1024).contains(&description.chars().count()),
        "description length"
    );
    assert!(
        description.contains("Use when"),
        "description says when to use the skill"
    );
    if let Some(c) = get("compatibility") {
        assert!(
            (1..=500).contains(&c.chars().count()),
            "compatibility length"
        );
    }
    let lines = read(&skill_dir().join("SKILL.md")).lines().count();
    assert!(lines < 500, "SKILL.md has {lines} lines; keep it under 500");
}

#[test]
fn skill_links_resolve() {
    let dir = skill_dir();
    let skill = read(&dir.join("SKILL.md"));
    for target in skill
        .split("](")
        .skip(1)
        .filter_map(|s| s.split(')').next())
    {
        if target.starts_with("http") || target.starts_with('#') {
            continue;
        }
        assert!(
            dir.join(target).is_file(),
            "SKILL.md links to missing `{target}`"
        );
    }
}

#[test]
fn skill_json_examples_render() {
    let mut blocks = Vec::new();
    for (path, text) in skill_markdown() {
        blocks.extend(json_blocks(&text).into_iter().map(|b| (path.clone(), b)));
    }
    let mut kinds = Vec::new();
    for (path, block) in &blocks {
        let at = path.display();
        let spec = Spec::from_json(block).unwrap_or_else(|e| panic!("{at}: {e}\n{block}"));
        let (_, report) = render(common::fixture(800, 600), &spec, None)
            .unwrap_or_else(|e| panic!("{at}: {e}\n{block}"));
        assert!(
            report.warnings.is_empty(),
            "{at}: {:?}\n{block}",
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
