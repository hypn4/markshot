use std::io::Write as _;
use std::path::PathBuf;
use std::process::ExitCode;

use clap::{Parser, Subcommand};
use markshot::error::Error;
use markshot::spec::Spec;
use serde::Serialize;

#[derive(Parser)]
#[command(version, about)]
struct Cli {
    #[command(subcommand)]
    cmd: Cmd,
}

#[derive(Subcommand)]
#[expect(
    dead_code,
    reason = "render and grid read their arguments once implemented"
)]
enum Cmd {
    /// Draw the annotations of a JSON spec on a PNG and print the resolved spec.
    Render {
        /// The screenshot (PNG).
        input: PathBuf,
        /// The annotation spec (JSON), or `-` for stdin.
        #[arg(long)]
        spec: PathBuf,
        /// Where to write the annotated PNG.
        #[arg(short, long)]
        out: PathBuf,
        /// Device pixel ratio; overrides the spec's `dpr` and the PNG's DPI.
        #[arg(long)]
        dpr: Option<f64>,
    },
    /// Overlay a labeled pixel grid on a PNG, for reading coordinates.
    Grid {
        /// The screenshot (PNG).
        input: PathBuf,
        /// Where to write the grid PNG.
        #[arg(short, long)]
        out: PathBuf,
        /// Grid spacing in pixels.
        #[arg(long)]
        step: Option<u32>,
    },
    /// Print the JSON schema of the annotation spec.
    Schema,
}

fn main() -> ExitCode {
    match run(&Cli::parse().cmd) {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("error: {e}");
            ExitCode::FAILURE
        }
    }
}

fn run(cmd: &Cmd) -> Result<(), Error> {
    match cmd {
        Cmd::Render { .. } => Err(Error::Unsupported("render".to_owned())),
        Cmd::Grid { .. } => Err(Error::Unsupported("grid".to_owned())),
        Cmd::Schema => print_json(&schemars::schema_for!(Spec)),
    }
}

/// Writes `value` to stdout as pretty JSON and a newline.
fn print_json(value: &impl Serialize) -> Result<(), Error> {
    let mut stdout = std::io::stdout().lock();
    serde_json::to_writer_pretty(&mut stdout, value).map_err(std::io::Error::from)?;
    writeln!(stdout)?;
    Ok(())
}
