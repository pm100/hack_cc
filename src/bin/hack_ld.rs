/// hack_ld — Hack linker.
///
/// Links one or more `.s` object files (produced by `hack_cc -c`) into
/// a final executable.
///
/// Usage:
///   hack_ld [options] file1.s [file2.s ...]
///
/// Options:
///   -o <output>       Output file (default: derived from first input)
///   -L <dir>          Library search directory (default: auto-discovered)
///   -f, --format      Output format: asm (default), hackem, hack, tst
///
/// The linker:
///   1. Reads all .s files and parses their `.provides` and `.data` directives.
///   2. Merges their `.data` entries (symbolic name-value pairs) in file order.
///   3. Generates a bootstrap (SP init, data init code, call main, halt).
///   4. Runs the runtime symbol-scan linker to pull in needed library modules.
///   5. Emits the requested output format.

use clap::Parser;
use std::path::PathBuf;
use hack_cc::output::{OutputFormat, emit};
use hack_cc::codegen::{gen_bootstrap, gen_font_init_asm, gen_font_data_inits, CompiledProgram, DataInit};
use hack_cc::linker::{link, default_lib_dirs};

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
#[command(name = "hack_ld", about = "Linker for Hack object files (.s)")]
struct Cli {
    /// Input .s object files (produced by hack_cc -c)
    #[arg(required = true)]
    inputs: Vec<PathBuf>,
    /// Library search directories (may be repeated)
    #[arg(short = 'L', value_name = "DIR")]
    lib_dirs: Vec<PathBuf>,
    /// Output file (default: derived from first input name)
    #[arg(short, long)]
    output: Option<PathBuf>,
    /// Output format (inferred from -o extension if not specified)
    #[arg(short, long, value_enum)]
    format: Option<Format>,
}

/// Metadata parsed from the leading directive lines of a `.s` object file.
struct ParsedSFile {
    /// Symbolic name-value pairs from `.data name val` directives.
    data: Vec<(String, i16)>,
    /// Full file text (directive lines are valid assembler no-ops, pass-through).
    asm_text: String,
}

fn parse_s_file(text: String) -> ParsedSFile {
    let mut data = Vec::new();
    for line in text.lines() {
        if let Some(rest) = line.strip_prefix(".data ") {
            let mut parts = rest.split_whitespace();
            if let (Some(name), Some(val_str)) = (parts.next(), parts.next()) {
                // Skip absolute font-table entries (format: `.data @addr val`)
                if name.starts_with('@') { continue; }
                if let Ok(v) = val_str.parse::<i16>() {
                    data.push((name.to_string(), v));
                }
            }
        }
    }
    ParsedSFile { data, asm_text: text }
}

/// Generate Hack assembly init code from symbolic name-value pairs.
/// Zero-value entries just emit `@sym` to force assembler allocation.
fn gen_init_code(data: &[(String, i16)]) -> String {
    let mut out = String::new();
    for (name, val) in data {
        let v = *val;
        if v == 0 {
            out.push_str(&format!("@{}\n", name));
        } else if v == 1 {
            out.push_str(&format!("D=1\n@{}\nM=D\n", name));
        } else if v == -1 {
            out.push_str(&format!("D=-1\n@{}\nM=D\n", name));
        } else if v > 0 {
            out.push_str(&format!("@{}\nD=A\n@{}\nM=D\n", v, name));
        } else {
            out.push_str(&format!("@{}\nD=-A\n@{}\nM=D\n", -(v as i32), name));
        }
    }
    out
}

fn main() {
    let cli = Cli::parse();

    let lib_dirs = if cli.lib_dirs.is_empty() {
        default_lib_dirs()
    } else {
        cli.lib_dirs.clone()
    };

    // ── Determine output format early (affects how font table is handled) ────
    let fmt_enum = cli.format.clone().or_else(|| {
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

    // ── Step 1: Read and parse all .s files ─────────────────────────────────
    let mut parsed: Vec<ParsedSFile> = Vec::new();
    for path in &cli.inputs {
        let text = std::fs::read_to_string(path).unwrap_or_else(|e| {
            eprintln!("error reading {:?}: {}", path, e);
            std::process::exit(1);
        });
        parsed.push(parse_s_file(text));
    }

    // ── Step 2: Merge .data entries from all files (in file order) ──────────
    let mut combined_data: Vec<(String, i16)> = Vec::new();
    for sf in &parsed {
        for entry in &sf.data {
            combined_data.push(entry.clone());
        }
    }

    // ── Step 3: Build combined ASM with bootstrap containing init code ───────
    let init_code = gen_init_code(&combined_data);

    let build_combined = |extra_init: &str| -> String {
        let full_init = if extra_init.is_empty() {
            init_code.clone()
        } else {
            format!("{}\n{}", extra_init, init_code)
        };
        let bootstrap = gen_bootstrap(&full_init);
        let mut combined = bootstrap;
        for sf in &parsed {
            combined.push('\n');
            combined.push_str(&sf.asm_text);
        }
        combined
    };

    // ── Step 4: Runtime symbol-scan linker ──────────────────────────────────
    // First pass: link without font init to discover which library modules are needed.
    let first_pass = link(&build_combined(""), &lib_dirs);
    let needs_font = first_pass.contains("(__draw_char)");

    // For hackem/tst output the font table is pre-loaded as static RAM@ data
    // (no bootstrap ASM needed).  For asm/hack output it must be inlined.
    let (linked_asm, font_data): (String, Vec<DataInit>) = if needs_font {
        match fmt_enum {
            Format::Hackem | Format::Tst => {
                // Font goes into RAM@ sections — no inline init ASM required.
                (first_pass, gen_font_data_inits())
            }
            _ => {
                // Font init as bootstrap ASM for formats without a RAM@ section.
                (link(&build_combined(&gen_font_init_asm()), &lib_dirs), Vec::new())
            }
        }
    } else {
        (first_pass, Vec::new())
    };

    // ── Step 5: Wrap into CompiledProgram and emit ───────────────────────────
    let prog = CompiledProgram { asm: linked_asm, data: font_data };

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
