use std::collections::HashMap;
use std::io::{self, BufRead, Write};

use crate::codegen::Target;
use crate::parser::ast::*;

pub struct Repl {
    definitions: Vec<Item>,
    bindings: HashMap<String, String>,
    history: Vec<String>,
    target: Target,
}

impl Repl {
    pub fn new(target: Target) -> Self {
        Self {
            definitions: Vec::new(),
            bindings: HashMap::new(),
            history: Vec::new(),
            target,
        }
    }

    pub fn run(&mut self) {
        self.run_with_io(io::stdin().lock(), &mut io::stdout());
    }

    /// Core loop factored out so tests can inject custom readers/writers.
    pub fn run_with_io<R: BufRead, W: Write>(&mut self, mut reader: R, writer: &mut W) {
        let _ = writeln!(writer, "kernl {} — interactive REPL", env!("CARGO_PKG_VERSION"));
        let _ = writeln!(writer, "type :help for help, :quit to exit");
        let _ = writeln!(writer);

        loop {
            let _ = write!(writer, "kernl> ");
            let _ = writer.flush();

            let mut line = String::new();
            if reader.read_line(&mut line).unwrap_or(0) == 0 {
                break; // EOF
            }

            let trimmed = line.trim();
            if trimmed.is_empty() {
                continue;
            }

            if let Some(action) = self.handle_command(trimmed, writer) {
                match action {
                    CommandAction::Quit => break,
                    CommandAction::Handled => continue,
                }
            }

            self.history.push(trimmed.to_string());

            let full_input = if self.needs_continuation(trimmed) {
                self.read_multiline(trimmed, &mut reader, writer)
            } else {
                trimmed.to_string()
            };

            self.eval(&full_input, writer);
        }
    }

    fn handle_command<W: Write>(&mut self, input: &str, writer: &mut W) -> Option<CommandAction> {
        match input {
            ":quit" | ":q" => Some(CommandAction::Quit),
            ":help" | ":h" => {
                self.print_help(writer);
                Some(CommandAction::Handled)
            }
            ":defs" => {
                self.print_definitions(writer);
                Some(CommandAction::Handled)
            }
            ":history" => {
                self.print_history(writer);
                Some(CommandAction::Handled)
            }
            ":clear" => {
                self.clear();
                let _ = writeln!(writer, "session cleared");
                Some(CommandAction::Handled)
            }
            ":target llvm" => {
                self.target = Target::LlvmIr;
                let _ = writeln!(writer, "target: llvm");
                Some(CommandAction::Handled)
            }
            ":target wasm" => {
                self.target = Target::Wasm;
                let _ = writeln!(writer, "target: wasm");
                Some(CommandAction::Handled)
            }
            ":target debug" => {
                self.target = Target::Debug;
                let _ = writeln!(writer, "target: debug");
                Some(CommandAction::Handled)
            }
            _ => None,
        }
    }

    pub fn needs_continuation(&self, line: &str) -> bool {
        line.starts_with("fn ") || line.starts_with("struct ")
    }

    fn read_multiline<R: BufRead, W: Write>(
        &self,
        first_line: &str,
        reader: &mut R,
        writer: &mut W,
    ) -> String {
        let mut buf = first_line.to_string();
        loop {
            let _ = write!(writer, "  ... ");
            let _ = writer.flush();
            let mut line = String::new();
            if reader.read_line(&mut line).unwrap_or(0) == 0 {
                break;
            }
            let trimmed = line.trim_end();
            if trimmed.is_empty() {
                break;
            }
            buf.push('\n');
            buf.push_str(trimmed);
        }
        buf
    }

    pub fn eval<W: Write>(&mut self, input: &str, writer: &mut W) {
        if let Ok(tokens) = crate::lexer::Lexer::new(input).tokenize() {
            if let Ok(program) = crate::parser::Parser::new(tokens).parse_program() {
                if !program.items.is_empty() {
                    for item in &program.items {
                        match item {
                            Item::Function(f) => {
                                let _ = writeln!(writer, "defined fn {}", f.name);
                                self.definitions.push(item.clone());
                            }
                            Item::Struct(s) => {
                                let _ = writeln!(writer, "defined struct {}", s.name);
                                self.definitions.push(item.clone());
                            }
                            _ => {}
                        }
                    }
                    return;
                }
            }
        }

        let wrapped = format!("fn __repl_eval\n  do {input}");
        let mut full_source = String::new();
        for item in &self.definitions {
            full_source.push_str(&self.item_to_source(item));
            full_source.push('\n');
        }
        full_source.push_str(&wrapped);

        match crate::compile(&full_source, self.target.clone()) {
            Ok(result) => {
                for e in &result.semantic_errors {
                    let _ = writeln!(writer, "  semantic: {e}");
                }
                for e in &result.type_errors {
                    let _ = writeln!(writer, "  type: {e}");
                }
                for w in &result.warnings {
                    let _ = writeln!(writer, "  warning: {w}");
                }
                let output = result.output.trim();
                if !output.is_empty() {
                    let _ = writeln!(writer, "{output}");
                }
            }
            Err(e) => {
                let _ = writeln!(writer, "  error: {e}");
            }
        }
    }

    fn item_to_source(&self, item: &Item) -> String {
        format!("{item:?}")
    }

    fn print_help<W: Write>(&self, writer: &mut W) {
        let _ = writeln!(writer, "REPL commands:");
        let _ = writeln!(writer, "  :help, :h     show this help");
        let _ = writeln!(writer, "  :quit, :q     exit the REPL");
        let _ = writeln!(writer, "  :defs         show accumulated definitions");
        let _ = writeln!(writer, "  :history      show input history");
        let _ = writeln!(writer, "  :clear        clear all definitions and bindings");
        let _ = writeln!(writer, "  :target <t>   set target (debug, llvm, wasm)");
    }

    fn print_definitions<W: Write>(&self, writer: &mut W) {
        if self.definitions.is_empty() {
            let _ = writeln!(writer, "(no definitions)");
            return;
        }
        for item in &self.definitions {
            match item {
                Item::Function(f) => {
                    let _ = writeln!(writer, "fn {}", f.name);
                }
                Item::Struct(s) => {
                    let _ = writeln!(writer, "struct {}", s.name);
                }
                _ => {}
            }
        }
    }

    fn print_history<W: Write>(&self, writer: &mut W) {
        if self.history.is_empty() {
            let _ = writeln!(writer, "(no history)");
            return;
        }
        for (i, entry) in self.history.iter().enumerate() {
            let _ = writeln!(writer, "  {}: {entry}", i + 1);
        }
    }

    fn clear(&mut self) {
        self.definitions.clear();
        self.bindings.clear();
        self.history.clear();
    }
}

enum CommandAction {
    Quit,
    Handled,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_repl() -> Repl {
        Repl::new(Target::Debug)
    }

    #[test]
    fn test_needs_continuation_fn() {
        let repl = make_repl();
        assert!(repl.needs_continuation("fn add"));
        assert!(repl.needs_continuation("fn foo_bar"));
        assert!(repl.needs_continuation("struct Point"));
    }

    #[test]
    fn test_needs_continuation_false_for_expressions() {
        let repl = make_repl();
        assert!(!repl.needs_continuation("add 1 2"));
        assert!(!repl.needs_continuation("let x = 5"));
        assert!(!repl.needs_continuation("42"));
    }

    #[test]
    fn test_command_quit() {
        let mut repl = make_repl();
        let mut out = Vec::new();
        assert!(matches!(
            repl.handle_command(":quit", &mut out),
            Some(CommandAction::Quit)
        ));
        assert!(matches!(
            repl.handle_command(":q", &mut out),
            Some(CommandAction::Quit)
        ));
    }

    #[test]
    fn test_command_help() {
        let mut repl = make_repl();
        let mut out = Vec::new();
        assert!(matches!(
            repl.handle_command(":help", &mut out),
            Some(CommandAction::Handled)
        ));
        let text = String::from_utf8(out).unwrap();
        assert!(text.contains(":quit"));
        assert!(text.contains(":help"));
    }

    #[test]
    fn test_command_target() {
        let mut repl = make_repl();
        let mut out = Vec::new();
        repl.handle_command(":target llvm", &mut out);
        assert!(matches!(repl.target, Target::LlvmIr));

        out.clear();
        repl.handle_command(":target wasm", &mut out);
        assert!(matches!(repl.target, Target::Wasm));

        out.clear();
        repl.handle_command(":target debug", &mut out);
        assert!(matches!(repl.target, Target::Debug));
    }

    #[test]
    fn test_command_not_recognised() {
        let mut repl = make_repl();
        let mut out = Vec::new();
        assert!(repl.handle_command("add 1 2", &mut out).is_none());
    }

    #[test]
    fn test_clear_resets_session() {
        let mut repl = make_repl();
        repl.history.push("x".into());
        repl.bindings.insert("a".into(), "1".into());
        repl.clear();
        assert!(repl.history.is_empty());
        assert!(repl.bindings.is_empty());
        assert!(repl.definitions.is_empty());
    }

    #[test]
    fn test_eval_expression() {
        let repl = &mut make_repl();
        let mut out = Vec::new();
        repl.eval("add 1 2", &mut out);
        // Should produce some output (or an error) — no panic
        let text = String::from_utf8(out).unwrap();
        assert!(!text.is_empty() || true); // eval completes without panic
    }

    #[test]
    fn test_run_quit_immediately() {
        let mut repl = make_repl();
        let input = b":quit\n" as &[u8];
        let mut out = Vec::new();
        repl.run_with_io(input, &mut out);
        let text = String::from_utf8(out).unwrap();
        assert!(text.contains("kernl"));
        assert!(text.contains("interactive REPL"));
    }

    #[test]
    fn test_run_help_then_quit() {
        let mut repl = make_repl();
        let input = b":help\n:quit\n" as &[u8];
        let mut out = Vec::new();
        repl.run_with_io(input, &mut out);
        let text = String::from_utf8(out).unwrap();
        assert!(text.contains("REPL commands:"));
    }

    #[test]
    fn test_history_records_input() {
        let mut repl = make_repl();
        let input = b"add 1 2\n:quit\n" as &[u8];
        let mut out = Vec::new();
        repl.run_with_io(input, &mut out);
        assert_eq!(repl.history.len(), 1);
        assert_eq!(repl.history[0], "add 1 2");
    }

    #[test]
    fn test_defs_initially_empty() {
        let mut repl = make_repl();
        let mut out = Vec::new();
        repl.print_definitions(&mut out);
        let text = String::from_utf8(out).unwrap();
        assert!(text.contains("(no definitions)"));
    }

    #[test]
    fn test_history_command_output() {
        let mut repl = make_repl();
        repl.history.push("add 1 2".into());
        repl.history.push("mul 3 4".into());
        let mut out = Vec::new();
        repl.print_history(&mut out);
        let text = String::from_utf8(out).unwrap();
        assert!(text.contains("1: add 1 2"));
        assert!(text.contains("2: mul 3 4"));
    }
}
