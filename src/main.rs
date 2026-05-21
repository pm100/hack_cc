use clap::Parser;
use std::collections::HashMap;
use std::path::PathBuf;
use hack_cc::output::{OutputFormat, emit};
use hack_cc::CompileOptions;

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
    /// Additional include search directories for #include <...> (may be repeated)
    #[arg(short = 'I', value_name = "DIR")]
    include_dirs: Vec<PathBuf>,
    /// Library search directories for the linker (may be repeated)
    #[arg(short = 'L', value_name = "DIR")]
    lib_dirs: Vec<PathBuf>,
    /// Output file (default: derived from first input name)
    #[arg(short, long)]
    output: Option<PathBuf>,
    /// Output format (inferred from -o extension if not specified)
    #[arg(short, long, value_enum)]
    format: Option<Format>,
    /// Compile only: produce a .s object file without linking or bootstrap.
    /// Use hack_ld to link one or more .s files into a final executable.
    #[arg(short = 'c', long = "compile-only")]
    compile_only: bool,
    /// Pre-define a macro (like -DNAME or -DNAME=VALUE).
    /// Use -D HACK_OUTPUT_SCREEN to select screen-buffer output.
    #[arg(short = 'D', value_name = "NAME[=VALUE]")]
    defines: Vec<String>,
    /// Write a map file to PATH. If PATH is omitted, uses the output base name
    /// with a .map extension.
    #[arg(long = "map", short = 'm', value_name = "PATH", num_args = 0..=1,
          default_missing_value = "")]
    map: Option<String>,
    /// Emit debug information alongside the output (.pdb file for source-level debugging).
    #[arg(short = 'g', long = "debug")]
    debug: bool,
}

fn main() {
    let cli = Cli::parse();

    if cli.compile_only {
        // -c mode: compile each input to a separate .s object file.
        if cli.output.is_some() && cli.inputs.len() > 1 {
            eprintln!("error: -o cannot be used with -c when compiling multiple files");
            std::process::exit(1);
        }
        // Build options (include dirs + defines) for -c mode.
        let mut c_defines: HashMap<String, String> = HashMap::new();
        for d in &cli.defines {
            if let Some((name, value)) = d.split_once('=') {
                c_defines.insert(name.to_string(), value.to_string());
            } else {
                c_defines.insert(d.clone(), "1".to_string());
            }
        }
        let c_opts = hack_cc::CompileOptions {
            include_dirs: cli.include_dirs.clone(),
            defines: c_defines,
            lib_dirs: Vec::new(), // not needed for object compilation
            debug: cli.debug,
        };
        for input in &cli.inputs {
            let src = std::fs::read_to_string(input).unwrap_or_else(|e| {
                eprintln!("error reading {:?}: {}", input, e);
                std::process::exit(1);
            });
            let debug_name = if cli.debug { input.to_str() } else { None };
            let obj_s = hack_cc::compile_to_object_with_options(&src, input.parent(), &c_opts, debug_name).unwrap_or_else(|e| {
                eprintln!("compile error in {:?}: {}", input, e);
                std::process::exit(1);
            });
            let out_path = if cli.inputs.len() == 1 {
                cli.output.clone().unwrap_or_else(|| input.with_extension("s"))
            } else {
                input.with_extension("s")
            };
            std::fs::write(&out_path, obj_s).unwrap_or_else(|e| {
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

    // Parse -D defines into a map.
    let mut defines: HashMap<String, String> = HashMap::new();
    for d in &cli.defines {
        if let Some((name, value)) = d.split_once('=') {
            defines.insert(name.to_string(), value.to_string());
        } else {
            defines.insert(d.clone(), "1".to_string());
        }
    }
    let lib_dirs = if cli.lib_dirs.is_empty() {
        hack_cc::linker::default_lib_dirs()
    } else {
        cli.lib_dirs.clone()
    };
    let opts = CompileOptions {
        include_dirs: cli.include_dirs.clone(),
        defines,
        lib_dirs,
        debug: cli.debug,
    };

    let prog = {
        let file_refs: Vec<(&str, Option<&std::path::Path>, Option<&str>)> = sources.iter()
            .map(|(src, path)| (src.as_str(), path.parent(), path.to_str()))
            .collect();
        hack_cc::compile_and_link(&file_refs, &opts, fmt)
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

    if cli.debug {
        hack_cc::write_pdb(&prog.asm, &sources, &cli.inputs, &out_path);
    }

    if let Some(map_arg) = &cli.map {
        let source_names: Vec<&str> = cli.inputs.iter()
            .map(|p| p.file_name().and_then(|n| n.to_str()).unwrap_or("?"))
            .collect();
        let map_text = hack_cc::mapfile::generate_map(&prog.asm, &source_names, &prog.data);
        let map_path = if map_arg.is_empty() {
            out_path.with_extension("map")
        } else {
            std::path::PathBuf::from(map_arg)
        };
        std::fs::write(&map_path, &map_text).unwrap_or_else(|e| {
            eprintln!("error writing map {:?}: {}", map_path, e);
            std::process::exit(1);
        });
        eprintln!("wrote map {:?}", map_path);
    }
}

