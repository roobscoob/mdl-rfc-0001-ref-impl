mod test_runner;

use std::path::Path;
use std::process;

use clap::{Parser, Subcommand};
use codespan_reporting::diagnostic::{Diagnostic, Label, Severity};
use codespan_reporting::files::SimpleFiles;
use codespan_reporting::term;
use codespan_reporting::term::termcolor::{ColorChoice, StandardStream};

use interpreter::{DiagnosticError, RuntimeValue};

const SUBCOMMANDS: &[&str] = &["run", "test", "help"];

#[derive(Parser)]
#[command(name = "mdl", version, about = "Markdownlang interpreter")]
struct Cli {
    /// Disable colored error output
    #[arg(long, global = true)]
    no_color: bool,

    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Run a markdownlang program
    Run(RunArgs),

    /// Run .test.md test files
    Test(TestArgs),
}

#[derive(clap::Args)]
struct RunArgs {
    /// Markdown source file to execute
    file: String,

    /// Entrypoint block name (case-insensitive)
    #[arg(short, long, default_value = "main")]
    entry: String,

    /// Parse only, don't execute (exit 0 if valid)
    #[arg(long)]
    check: bool,

    /// Dump parsed AST
    #[arg(long)]
    ast: bool,

    /// List all block names in the program
    #[arg(long)]
    list_blocks: bool,

    /// Suppress runtime output (just check for errors)
    #[arg(short, long)]
    quiet: bool,

    /// Arguments passed to the entrypoint block (after --)
    #[arg(last = true)]
    args: Vec<String>,
}

#[derive(clap::Args)]
struct TestArgs {
    /// Path to a .test.md file or directory containing them
    path: String,

    /// Run only tests in these categories (subfolder names). Repeatable.
    #[arg(short, long)]
    category: Vec<String>,

    /// List available categories and exit
    #[arg(long)]
    list_categories: bool,
}

fn main() {
    // Backwards compatibility: if the first positional arg is not a known
    // subcommand, inject "run" so `mdl file.md` works like `mdl run file.md`.
    let mut args: Vec<String> = std::env::args().collect();
    if let Some(first_pos) = args.iter().skip(1).find(|a| !a.starts_with('-')) {
        let first_pos = first_pos.clone();
        if !SUBCOMMANDS.contains(&first_pos.as_str()) {
            let pos = args.iter().position(|a| *a == first_pos).unwrap();
            args.insert(pos, "run".to_string());
        }
    }

    let cli = Cli::parse_from(&args);

    match cli.command {
        Command::Run(run_args) => do_run(run_args, cli.no_color),
        Command::Test(test_args) => {
            let path = Path::new(&test_args.path);
            if test_args.list_categories {
                test_runner::list_categories(path);
                return;
            }
            let exit_code = test_runner::run_tests(path, cli.no_color, &test_args.category);
            process::exit(exit_code);
        }
    }
}

fn do_run(args: RunArgs, no_color: bool) {
    let color_choice = if no_color {
        ColorChoice::Never
    } else {
        ColorChoice::Auto
    };

    // Read source
    let source = match std::fs::read_to_string(&args.file) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("error: cannot read '{}': {}", args.file, e);
            process::exit(1);
        }
    };

    // Set up codespan file database
    let mut files = SimpleFiles::new();
    let file_id = files.add(args.file.clone(), source.clone());

    // Parse
    let parser = mdl::parser::Parser::new(source, file_id);
    let program = match parser.parse() {
        Ok(p) => p,
        Err(errors) => {
            let writer = StandardStream::stderr(color_choice);
            let config = term::Config::default();
            for error in &errors {
                let diagnostic = error.to_diagnostic();
                let _ =
                    term::emit_to_write_style(&mut writer.lock(), &config, &files, &diagnostic);
            }
            process::exit(1);
        }
    };

    // --check: parse succeeded, exit
    if args.check {
        eprintln!("ok: {} parsed successfully", args.file);
        return;
    }

    // --ast: dump AST
    if args.ast {
        println!("{:#?}", program);
        return;
    }

    // --list-blocks: print all block names
    if args.list_blocks {
        fn print_blocks(blocks: &[mdl::block::Block], indent: usize) {
            for block in blocks {
                let prefix = "#".repeat(block.level as usize);
                let pad = "  ".repeat(indent);
                let has_chain = if block.chain.is_empty() {
                    "(document)"
                } else {
                    ""
                };
                println!("{}{} {} {}", pad, prefix, block.name, has_chain);
                print_blocks(&block.children, indent + 1);
            }
        }
        print_blocks(&program.blocks, 0);
        return;
    }

    // Determine base directory for imports
    let base_dir = Path::new(&args.file)
        .parent()
        .map(|p| p.to_path_buf())
        .unwrap_or_else(|| std::path::PathBuf::from("."));

    // Parse CLI arguments into RuntimeValues
    let arguments: Vec<RuntimeValue> = args.args.iter().map(|s| parse_arg(s)).collect();

    // Execute
    let result = if args.quiet {
        let mut sink = std::io::sink();
        interpreter::execute_program_entry(&program, &mut sink, base_dir, &args.entry, arguments)
    } else {
        let mut stdout = std::io::stdout();
        interpreter::execute_program_entry(&program, &mut stdout, base_dir, &args.entry, arguments)
    };

    let writer = StandardStream::stderr(color_choice);
    let config = term::Config::default();

    match result {
        Ok((_value, warnings)) => {
            emit_diagnostics(&writer, &config, &files, &warnings);
        }
        Err(error) => {
            emit_diagnostic_error(&writer, &config, &files, &error);
            process::exit(1);
        }
    }
}

fn emit_diagnostic_error(
    writer: &StandardStream,
    config: &term::Config,
    files: &SimpleFiles<String, String>,
    error: &DiagnosticError,
) {
    if let Some(span) = &error.span {
        let severity = if error.is_warning {
            Severity::Warning
        } else {
            Severity::Error
        };
        let diagnostic = Diagnostic::new(severity)
            .with_message(error.to_string())
            .with_labels(vec![Label::primary(error.source_id, span.clone())]);
        let _ = term::emit_to_write_style(&mut writer.lock(), config, files, &diagnostic);
    } else {
        let prefix = if error.is_warning {
            "warning"
        } else {
            "runtime error"
        };
        eprintln!("{}: {}", prefix, error);
    }
}

fn emit_diagnostics(
    writer: &StandardStream,
    config: &term::Config,
    files: &SimpleFiles<String, String>,
    diagnostics: &[DiagnosticError],
) {
    for diag in diagnostics {
        emit_diagnostic_error(writer, config, files, diag);
    }
}

/// Parse a CLI argument string into a RuntimeValue.
/// Numbers become Number, "true"/"false" become Boolean, everything else is String.
fn parse_arg(s: &str) -> RuntimeValue {
    if let Ok(n) = s.parse::<f64>() {
        return RuntimeValue::Number(n);
    }
    match s {
        "true" => RuntimeValue::Boolean(true),
        "false" => RuntimeValue::Boolean(false),
        _ => RuntimeValue::String(s.to_string()),
    }
}
