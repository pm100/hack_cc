use clap::Parser;
use std::path::PathBuf;

#[derive(Parser)]
#[command(name = "hack_cc", about = "C compiler targeting the Hack CPU (nand2tetris)")]
struct Cli {
    /// Input C source file
    input: PathBuf,
    /// Output Hack assembly file
    #[arg(short, long, default_value = "out.asm")]
    output: PathBuf,
}

fn main() {
    let cli = Cli::parse();
    let source = std::fs::read_to_string(&cli.input).unwrap_or_else(|e| {
        eprintln!("error reading {:?}: {}", cli.input, e);
        std::process::exit(1);
    });
    match hack_cc::compile(&source) {
        Ok(asm) => {
            std::fs::write(&cli.output, asm).unwrap_or_else(|e| {
                eprintln!("error writing {:?}: {}", cli.output, e);
                std::process::exit(1);
            });
        }
        Err(e) => {
            eprintln!("compile error: {}", e);
            std::process::exit(1);
        }
    }
}
