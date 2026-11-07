use std::fs;
use std::io::{Read as _, Write as _};
use std::path::{Path, PathBuf};
use std::process::ExitCode;

use clap::{Parser, Subcommand};
use markshot::error::Error;
use markshot::image_io::decode_png;
use markshot::spec::Spec;
use serde::Serialize;

#[derive(Parser)]
#[command(version, about)]
struct Cli {
    #[command(subcommand)]
    cmd: Cmd,
}

#[derive(Subcommand)]
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
        /// Device pixel ratio, 0.5 to 4; overrides the spec's `dpr` and the PNG's DPI.
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
        /// Grid spacing in pixels, at least 1; by default the smallest of 50, 100, 200, 250,
        /// 500 and 1000 that gives at most 20 intervals along the longer side.
        #[arg(long)]
        step: Option<u32>,
        /// Draw only this part of the image, `X,Y,W,H` in image pixels, with the grid of the
        /// whole image: for reading a small area at full size.
        #[arg(long, value_name = "X,Y,W,H")]
        region: Option<String>,
    },
    /// Print the JSON schema of the annotation spec.
    Schema,
}

fn main() -> ExitCode {
    let cli = match Cli::try_parse() {
        Ok(cli) => cli,
        // Every input error exits 1, usage errors included (clap's own code is 2);
        // `--help` and `--version` print to stdout and exit 0.
        Err(e) => {
            // Nothing useful is left to do if printing fails.
            let _ = e.print();
            return if e.use_stderr() {
                ExitCode::FAILURE
            } else {
                ExitCode::SUCCESS
            };
        }
    };
    match run(&cli.cmd) {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("error: {e}");
            ExitCode::FAILURE
        }
    }
}

fn run(cmd: &Cmd) -> Result<(), Error> {
    match cmd {
        Cmd::Render {
            input,
            spec,
            out,
            dpr,
        } => render(input, spec, out, *dpr),
        Cmd::Grid {
            input,
            out,
            step,
            region,
        } => grid(input, out, *step, region.as_deref()),
        Cmd::Schema => print_json(&schemars::schema_for!(Spec)),
    }
}

/// Draws the spec on the input, writes the PNG, then prints the report.
fn render(input: &Path, spec: &Path, out: &Path, dpr: Option<f64>) -> Result<(), Error> {
    let src = decode_png(&fs::read(input).map_err(file_error(input))?)?;
    let spec = Spec::from_json(&read_spec(spec)?)?;
    let (png, report) = markshot::render::render(src, &spec, dpr)?;
    fs::write(out, png).map_err(file_error(out))?;
    print_json(&report)
}

/// Draws the grid on the input, or on the part `region` gives, and writes the PNG.
fn grid(input: &Path, out: &Path, step: Option<u32>, region: Option<&str>) -> Result<(), Error> {
    let region = region.map(parse_region).transpose()?;
    let src = decode_png(&fs::read(input).map_err(file_error(input))?)?;
    let png = markshot::grid::grid(src, step, region)?;
    fs::write(out, png).map_err(file_error(out))
}

/// `X,Y,W,H` as four whole numbers.
///
/// # Errors
/// [`Error::Spec`] at `--region` otherwise.
fn parse_region(text: &str) -> Result<[u32; 4], Error> {
    let error = || Error::Spec {
        path: "--region".to_owned(),
        message: format!("must be X,Y,W,H in whole pixels, got `{text}`"),
    };
    let values: Vec<u32> = text
        .split(',')
        .map(|v| v.trim().parse().map_err(|_| error()))
        .collect::<Result<_, _>>()?;
    <[u32; 4]>::try_from(values).map_err(|_| error())
}

/// The spec text from `path`, or from stdin when `path` is `-`.
fn read_spec(path: &Path) -> Result<String, Error> {
    if path == Path::new("-") {
        let mut text = String::new();
        std::io::stdin().read_to_string(&mut text)?;
        Ok(text)
    } else {
        fs::read_to_string(path).map_err(file_error(path))
    }
}

/// Turns an I/O error on `path` into one whose message starts with the path.
fn file_error(path: &Path) -> impl FnOnce(std::io::Error) -> Error {
    move |e| {
        Error::Io(std::io::Error::new(
            e.kind(),
            format!("{}: {e}", path.display()),
        ))
    }
}

/// Writes `value` to stdout as pretty JSON and a newline.
fn print_json(value: &impl Serialize) -> Result<(), Error> {
    let mut stdout = std::io::stdout().lock();
    serde_json::to_writer_pretty(&mut stdout, value).map_err(std::io::Error::from)?;
    writeln!(stdout)?;
    Ok(())
}
