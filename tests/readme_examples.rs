//! The specs behind the README's example images, `assets/examples/*.json`, stay valid: each
//! parses and resolves for a screenshot the size of the 2800×1890 captures they were drawn on.
//! The captures themselves are not in the repository, so the images are not re-rendered here.

use std::fs;
use std::path::PathBuf;

use markshot::resolve::resolve;
use markshot::spec::Spec;

#[test]
fn readme_example_specs_resolve() {
    let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("assets/examples");
    let mut specs: Vec<PathBuf> = fs::read_dir(&dir)
        .unwrap()
        .map(|entry| entry.unwrap().path())
        .filter(|path| path.extension().is_some_and(|ext| ext == "json"))
        .collect();
    specs.sort();
    assert!(!specs.is_empty(), "no specs in {}", dir.display());
    for path in specs {
        let at = path.display();
        let text = fs::read_to_string(&path).unwrap();
        let spec = Spec::from_json(&text).unwrap_or_else(|e| panic!("{at}: {e}"));
        let image = path.with_extension("png");
        assert!(image.is_file(), "{at} has no image next to it");
        resolve(&spec, 2800, 1890, 1.0, None).unwrap_or_else(|e| panic!("{at}: {e}"));
    }
}
