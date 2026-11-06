//! Failures are reported for agents: `error: <message>` on stderr, nothing on stdout, exit 1.

use assert_cmd::cargo::cargo_bin_cmd;

#[test]
fn failure_prints_error_to_stderr_and_exits_1() {
    let output = cargo_bin_cmd!("markshot")
        .args([
            "render",
            "no-such-input.png",
            "--spec",
            "no-such-spec.json",
            "-o",
            "no-such-output.png",
        ])
        .output()
        .expect("markshot runs");
    assert_eq!(output.status.code(), Some(1), "{output:?}");
    assert!(output.stdout.is_empty(), "{output:?}");
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.starts_with("error: "), "{stderr}");
}
