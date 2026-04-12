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
    /// Input C source file(s). Multiple files are parsed independently and
    /// merged before compilation (like a simple linker).
    #[arg(required = true)]
    inputs: Vec<PathBuf>,
    /// Output file (default: derived from first input name)
    #[arg(short, long)]
    output: Option<PathBuf>,
    /// Output format (inferred from -o extension if not specified)
    #[arg(short, long, value_enum)]
    format: Option<Format>,
    /// Compile only: produce a .hobj object file without linking or bootstrap.
    /// Use hack_ld to link one or more .hobj files into a final executable.
    #[arg(short = 'c', long = "compile-only")]
    compile_only: bool,
}

fn main() {
    let cli = Cli::parse();

    if cli.compile_only {
        // -c mode: compile each input to a separate .hobj file.
        if cli.output.is_some() && cli.inputs.len() > 1 {
            eprintln!("error: -o cannot be used with -c when compiling multiple files");
            std::process::exit(1);
        }
        for input in &cli.inputs {
            let src = std::fs::read_to_string(input).unwrap_or_else(|e| {
                eprintln!("error reading {:?}: {}", input, e);
                std::process::exit(1);
            });
            let obj = hack_cc::compile_to_object(&src, input.parent()).unwrap_or_else(|e| {
                eprintln!("compile error in {:?}: {}", input, e);
                std::process::exit(1);
            });
            let out_path = if cli.inputs.len() == 1 {
                cli.output.clone().unwrap_or_else(|| input.with_extension("hobj"))
            } else {
                input.with_extension("hobj")
            };
            std::fs::write(&out_path, obj.serialize()).unwrap_or_else(|e| {
                eprintln!("error writing {:?}: {}", out_path, e);
                std::process::exit(1);
            });
        }
        return;
    }

    // Read all input files up front so errors are reported before compilation.
    let sources: Vec<(String, PathBuf)> = cli.inputs.iter().map(|p| {
        let src = std::fs::read_to_string(p).unwrap_or_else(|e| {
            eprintln!("error reading {:?}: {}", p, e);
            std::process::exit(1);
        });
        (src, p.clone())
    }).collect();

    // Infer format from flag or output extension.
    let fmt_enum = cli.format.or_else(|| {
        cli.output.as_ref().and_then(|p| {
            match p.extension().and_then(|e| e.to_str()) {
                Some("hackem") => Some(Format::Hackem),
                Some("hack")   => Some(Format::Hack),
                Some("tst")    => Some(Format::Tst),
                Some("asm")    => Some(Format::Asm),
                _              => None,
            }
        })
    }).unwrap_or(Format::Asm);

    let fmt = match fmt_enum {
        Format::Asm    => OutputFormat::Asm,
        Format::Hackem => OutputFormat::Hackem,
        Format::Hack   => OutputFormat::Hack,
        Format::Tst    => OutputFormat::Tst,
    };

    let prog = if sources.len() == 1 {
        let (src, path) = &sources[0];
        hack_cc::compile_with_path(src, path.parent())
    } else {
        // Multi-file: pass each (source, base_dir) pair to compile_files.
        let file_refs: Vec<(&str, Option<&std::path::Path>)> = sources.iter()
            .map(|(src, path)| (src.as_str(), path.parent()))
            .collect();
        hack_cc::compile_files(&file_refs)
    }.unwrap_or_else(|e| {
        eprintln!("compile error: {}", e);
        std::process::exit(1);
    });

    let default_ext = match fmt {
        OutputFormat::Asm    => "asm",
        OutputFormat::Hackem => "hackem",
        OutputFormat::Hack   => "hack",
        OutputFormat::Tst    => "tst",
    };
    // Default output name derived from first input file.
    let out_path = cli.output.unwrap_or_else(|| {
        cli.inputs[0].with_extension(default_ext)
    });

    let result = emit(&prog, fmt).unwrap_or_else(|e| {
        eprintln!("output error: {}", e);
        std::process::exit(1);
    });

    std::fs::write(&out_path, &result.main).unwrap_or_else(|e| {
        eprintln!("error writing {:?}: {}", out_path, e);
        std::process::exit(1);
    });

    if let Some(hack_content) = result.hack_companion {
        let hack_path = out_path.with_extension("hack");
        std::fs::write(&hack_path, &hack_content).unwrap_or_else(|e| {
            eprintln!("error writing companion {:?}: {}", hack_path, e);
            std::process::exit(1);
        });
        println!("wrote {:?} and {:?}", out_path, hack_path);
    }
}
