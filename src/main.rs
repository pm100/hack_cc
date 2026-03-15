use clap::Parser;
use std::path::PathBuf;
use hack_cc::output::{OutputFormat, emit};

#[derive(clap::ValueEnum, Clone, Debug)]
enum Format {
    /// Hack assembly text with bootstrap data-init code (default)
    Asm,
    /// hackem binary: ROM@ code section + RAM@ data sections
    Hackem,
    /// nand2tetris .hack binary: 16-bit binary strings, data in bootstrap code
    Hack,
    /// nand2tetris test script (.tst) with set RAM[] preamble + companion .hack binary
    Tst,
}

#[derive(Parser)]
#[command(name = "hack_cc", about = "C compiler targeting the Hack CPU (nand2tetris)")]
struct Cli {
    /// Input C source file
    input: PathBuf,
    /// Output file (default: derived from input name)
    #[arg(short, long)]
    output: Option<PathBuf>,
    /// Output format
    #[arg(short, long, value_enum, default_value = "asm")]
    format: Format,
}

fn main() {
    let cli = Cli::parse();

    let source = std::fs::read_to_string(&cli.input).unwrap_or_else(|e| {
        eprintln!("error reading {:?}: {}", cli.input, e);
        std::process::exit(1);
    });

    let prog = hack_cc::compile(&source).unwrap_or_else(|e| {
        eprintln!("compile error: {}", e);
        std::process::exit(1);
    });

    let fmt = match cli.format {
        Format::Asm    => OutputFormat::Asm,
        Format::Hackem => OutputFormat::Hackem,
        Format::Hack   => OutputFormat::Hack,
        Format::Tst    => OutputFormat::Tst,
    };

    // Derive default output path from input, using format-appropriate extension
    let default_ext = match fmt {
        OutputFormat::Asm    => "asm",
        OutputFormat::Hackem => "hackem",
        OutputFormat::Hack   => "hack",
        OutputFormat::Tst    => "tst",
    };
    let out_path = cli.output.unwrap_or_else(|| {
        cli.input.with_extension(default_ext)
    });

    let result = emit(&prog, fmt).unwrap_or_else(|e| {
        eprintln!("output error: {}", e);
        std::process::exit(1);
    });

    std::fs::write(&out_path, &result.main).unwrap_or_else(|e| {
        eprintln!("error writing {:?}: {}", out_path, e);
        std::process::exit(1);
    });

    // For tst format, also write the companion .hack file
    if let Some(hack_content) = result.hack_companion {
        let hack_path = out_path.with_extension("hack");
        std::fs::write(&hack_path, &hack_content).unwrap_or_else(|e| {
            eprintln!("error writing companion {:?}: {}", hack_path, e);
            std::process::exit(1);
        });
        println!("wrote {:?} and {:?}", out_path, hack_path);
    }
}
