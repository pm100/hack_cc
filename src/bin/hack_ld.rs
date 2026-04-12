/// hack_ld — Hack linker.
///
/// Links one or more `.hobj` object files (produced by `hack_cc -c`) into
/// a final executable.
///
/// Usage:
///   hack_ld [options] file1.hobj [file2.hobj ...]
///
/// Options:
///   -o <output>     Output file (default: derived from first input)
///   -f, --format    Output format: asm (default), hackem, hack, tst
///
/// The linker:
///   1. Reads all .hobj files.
///   2. Merges their ASM bodies and DataInit entries.
///      (Warns if two TUs have overlapping DataInit addresses — use
///       `hack_cc file1.c file2.c` instead for programs with cross-file globals.)
///   3. Generates a bootstrap (SP init, call main, halt) with the combined
///      data initialisations spliced in.
///   4. Runs the runtime symbol-scan linker to pull in needed runtime modules.
///   5. Emits the requested output format.

use clap::Parser;
use std::path::PathBuf;
use hack_cc::output::{OutputFormat, emit};
use hack_cc::codegen::{gen_bootstrap, CompiledProgram, DataInit};
use hack_cc::linker::link;
use hack_cc::object::ObjectFile;

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
#[command(name = "hack_ld", about = "Linker for Hack object files (.hobj)")]
struct Cli {
    /// Input .hobj object files
    #[arg(required = true)]
    inputs: Vec<PathBuf>,
    /// Output file (default: derived from first input name)
    #[arg(short, long)]
    output: Option<PathBuf>,
    /// Output format (inferred from -o extension if not specified)
    #[arg(short, long, value_enum)]
    format: Option<Format>,
}

fn main() {
    let cli = Cli::parse();

    // ── Step 1: Read and parse all .hobj files ───────────────────────────────
    let mut objects: Vec<ObjectFile> = Vec::new();
    for path in &cli.inputs {
        let text = std::fs::read_to_string(path).unwrap_or_else(|e| {
            eprintln!("error reading {:?}: {}", path, e);
            std::process::exit(1);
        });
        let obj = ObjectFile::parse(&text).unwrap_or_else(|e| {
            eprintln!("error parsing {:?}: {}", path, e);
            std::process::exit(1);
        });
        objects.push(obj);
    }

    // ── Step 2: Merge DataInit — detect conflicts ────────────────────────────
    let mut combined_data: Vec<DataInit> = Vec::new();
    let mut seen_addrs: std::collections::HashMap<u16, (usize, i16)> = std::collections::HashMap::new();
    for (file_idx, obj) in objects.iter().enumerate() {
        for d in &obj.data {
            if let Some(&(prev_idx, prev_val)) = seen_addrs.get(&d.address) {
                if prev_val != d.value {
                    eprintln!(
                        "warning: DataInit conflict at RAM[{}]: \
                         file {} sets {}, file {} sets {} — \
                         programs with global variables across separately-compiled TUs \
                         should be built with `hack_cc file1.c file2.c ...` instead.",
                        d.address, prev_idx, prev_val, file_idx, d.value
                    );
                }
                // Keep first definition, skip duplicate.
            } else {
                seen_addrs.insert(d.address, (file_idx, d.value));
                combined_data.push(d.clone());
            }
        }
    }

    // ── Step 3: Build combined ASM ───────────────────────────────────────────
    let bootstrap = gen_bootstrap();
    let mut combined_asm = bootstrap;
    for obj in &objects {
        combined_asm.push('\n');
        combined_asm.push_str(&obj.asm_body);
    }

    // ── Step 4: Runtime symbol-scan linker ──────────────────────────────────
    let linked_asm = link(&combined_asm);

    // ── Step 5: Wrap into CompiledProgram and emit ───────────────────────────
    let prog = CompiledProgram {
        asm: linked_asm,
        data: combined_data,
    };

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

    let default_ext = match fmt {
        OutputFormat::Asm    => "asm",
        OutputFormat::Hackem => "hackem",
        OutputFormat::Hack   => "hack",
        OutputFormat::Tst    => "tst",
    };
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
    } else {
        eprintln!("wrote {:?}", out_path);
    }
}
