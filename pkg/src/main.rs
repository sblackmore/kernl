mod commands;
mod manifest;
mod registry;
mod resolver;

use std::env;
use std::process;

fn main() {
    let args: Vec<String> = env::args().collect();

    if args.len() < 2 {
        print_usage();
        process::exit(1);
    }

    let result = match args[1].as_str() {
        "init" => commands::init::run(args.get(2).map(|s| s.as_str())),
        "build" => {
            let target = parse_target_flag(&args).unwrap_or(commands::build::Target::Llvm);
            commands::build::run(target)
        }
        "run" => commands::run::run(args.get(2).map(|s| s.as_str())),
        "check" => commands::check::run(),
        "add" => {
            let name = match args.get(2) {
                Some(n) => n.as_str(),
                None => {
                    eprintln!("error: `kernl add` requires a package name");
                    eprintln!("usage: kernl add <name> [version]");
                    process::exit(1);
                }
            };
            let version = args.get(3).map(|s| s.as_str());
            commands::add::run(name, version)
        }
        "info" => commands::info::run(),
        "search" => {
            let query = match args.get(2) {
                Some(q) => q.as_str(),
                None => {
                    eprintln!("error: `kernl search` requires a query");
                    eprintln!("usage: kernl search <query>");
                    process::exit(1);
                }
            };
            commands::search::run(query)
        }
        "publish" => commands::publish::run(),
        "install" => commands::install::run(),
        "help" | "--help" | "-h" => {
            print_usage();
            Ok(())
        }
        "version" | "--version" | "-V" => {
            println!("kernl {}", env!("CARGO_PKG_VERSION"));
            Ok(())
        }
        other => Err(format!("unknown command: {other}\nrun `kernl help` for usage")),
    };

    if let Err(e) = result {
        eprintln!("error: {e}");
        process::exit(1);
    }
}

fn parse_target_flag(args: &[String]) -> Option<commands::build::Target> {
    args.iter()
        .position(|a| a == "--target")
        .and_then(|i| args.get(i + 1))
        .map(|t| commands::build::Target::from_str(t))
        .transpose()
        .unwrap_or_else(|e| {
            eprintln!("error: {e}");
            process::exit(1);
        })
}

fn print_usage() {
    eprintln!("kernl — package manager for the kernl programming language");
    eprintln!();
    eprintln!("usage: kernl <command> [options]");
    eprintln!();
    eprintln!("commands:");
    eprintln!("  init [name]              create a new kernl project");
    eprintln!("  build [--target T]       compile the project (targets: llvm, wasm, debug)");
    eprintln!("  run [file]               compile and run a kernl program");
    eprintln!("  check                    parse and type-check without emitting code");
    eprintln!("  add <name> [version]     add a dependency to kernl.toml");
    eprintln!("  install                  resolve and install dependencies");
    eprintln!("  search <query>           search the package registry");
    eprintln!("  publish                  publish the package to the registry");
    eprintln!("  info                     display project info from kernl.toml");
    eprintln!("  help                     show this help message");
    eprintln!("  version                  show version");
}
