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

#[test]
fn usage_errors_exit_1() {
    // Spec §5.1 has two exit codes, 0 and 1; clap's own usage errors exit 2. Without a
    // subcommand clap prints the help instead of an `error:` line.
    for (args, error_line) in [
        ("render in.png --spec spec.json -o out.png --dpr abc", true),
        ("render in.png -o out.png", true),
        ("grid in.png -o out.png --step -5", true),
        ("", false),
    ] {
        let output = cargo_bin_cmd!("markshot")
            .args(args.split_whitespace())
            .output()
            .expect("markshot runs");
        assert_eq!(output.status.code(), Some(1), "{args:?}: {output:?}");
        assert!(output.stdout.is_empty(), "{args:?}: {output:?}");
        let stderr = String::from_utf8_lossy(&output.stderr);
        let want = if error_line { "error: " } else { "Render " };
        assert!(stderr.starts_with(want), "{args:?}: {stderr}");
    }
}

#[test]
fn help_and_version_exit_0() {
    for args in ["--help", "render --help", "--version"] {
        let output = cargo_bin_cmd!("markshot")
            .args(args.split_whitespace())
            .output()
            .expect("markshot runs");
        assert_eq!(output.status.code(), Some(0), "{args:?}: {output:?}");
        assert!(!output.stdout.is_empty(), "{args:?}: {output:?}");
    }
}
