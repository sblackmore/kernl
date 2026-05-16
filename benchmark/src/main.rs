use std::collections::BTreeMap;
use std::fs;
use std::path::Path;

fn main() {
    let programs_dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("programs");

    if !programs_dir.exists() {
        eprintln!("error: programs/ directory not found at {}", programs_dir.display());
        std::process::exit(1);
    }

    let groups = discover_program_groups(&programs_dir);
    if groups.is_empty() {
        eprintln!("no benchmark programs found in {}", programs_dir.display());
        std::process::exit(1);
    }

    println!("╔══════════════════════════════════════════════════════════════════════════════╗");
    println!("║                    kernl LLM Token Benchmark                                ║");
    println!("╠══════════════════════════════════════════════════════════════════════════════╣");
    println!("║  Compares equivalent programs across languages on metrics that matter       ║");
    println!("║  for LLM code generation: token count, character count, line count.         ║");
    println!("╚══════════════════════════════════════════════════════════════════════════════╝");
    println!();

    let mut totals: BTreeMap<String, Metrics> = BTreeMap::new();

    for (group_name, files) in &groups {
        println!("── {} ──", group_name);
        println!("{:<10} {:>8} {:>8} {:>8} {:>10}", "lang", "tokens", "chars", "lines", "approx_gpt");
        println!("{}", "─".repeat(50));

        for (lang, path) in files {
            let source = fs::read_to_string(path).unwrap();
            let m = analyze(&source);

            println!("{:<10} {:>8} {:>8} {:>8} {:>10}",
                lang, m.tokens, m.chars, m.lines, m.approx_gpt_tokens);

            let entry = totals.entry(lang.clone()).or_default();
            entry.tokens += m.tokens;
            entry.chars += m.chars;
            entry.lines += m.lines;
            entry.approx_gpt_tokens += m.approx_gpt_tokens;
        }
        println!();
    }

    println!("══ TOTALS ══");
    println!("{:<10} {:>8} {:>8} {:>8} {:>10}", "lang", "tokens", "chars", "lines", "approx_gpt");
    println!("{}", "─".repeat(50));

    let knl_gpt = totals.get("kernl").map(|m| m.approx_gpt_tokens).unwrap_or(1);

    for (lang, m) in &totals {
        let ratio = if *lang == "kernl" {
            "1.00x".to_string()
        } else {
            format!("{:.2}x", m.approx_gpt_tokens as f64 / knl_gpt as f64)
        };
        println!("{:<10} {:>8} {:>8} {:>8} {:>10}  {}",
            lang, m.tokens, m.chars, m.lines, m.approx_gpt_tokens, ratio);
    }

    println!();
    if let Some(knl) = totals.get("kernl") {
        for (lang, m) in &totals {
            if *lang != "kernl" {
                let saving = ((m.approx_gpt_tokens as f64 - knl.approx_gpt_tokens as f64) / m.approx_gpt_tokens as f64) * 100.0;
                println!("kernl saves ~{saving:.0}% tokens vs {lang}");
            }
        }
    }
}

#[derive(Default, Debug)]
struct Metrics {
    tokens: usize,
    chars: usize,
    lines: usize,
    approx_gpt_tokens: usize,
}

fn analyze(source: &str) -> Metrics {
    let source = source.trim();
    let chars = source.len();
    let lines = source.lines().count();

    let tokens = tokenize_code(source).len();

    // GPT-4 averages ~3.5 chars per token for code.
    // This is a well-known approximation from OpenAI's tokenizer research.
    let approx_gpt_tokens = (chars as f64 / 3.5).ceil() as usize;

    Metrics { tokens, chars, lines, approx_gpt_tokens }
}

/// Split source into whitespace/punctuation tokens (code-aware).
fn tokenize_code(source: &str) -> Vec<&str> {
    let mut tokens = Vec::new();
    let bytes = source.as_bytes();
    let mut i = 0;

    while i < bytes.len() {
        if bytes[i].is_ascii_whitespace() {
            i += 1;
            continue;
        }

        if bytes[i] == b'#' || (bytes[i] == b'/' && i + 1 < bytes.len() && bytes[i + 1] == b'/') {
            while i < bytes.len() && bytes[i] != b'\n' {
                i += 1;
            }
            continue;
        }

        if bytes[i] == b'"' {
            let start = i;
            i += 1;
            while i < bytes.len() && bytes[i] != b'"' {
                if bytes[i] == b'\\' { i += 1; }
                i += 1;
            }
            if i < bytes.len() { i += 1; }
            tokens.push(&source[start..i]);
            continue;
        }

        if bytes[i].is_ascii_alphanumeric() || bytes[i] == b'_' {
            let start = i;
            while i < bytes.len() && (bytes[i].is_ascii_alphanumeric() || bytes[i] == b'_') {
                i += 1;
            }
            tokens.push(&source[start..i]);
            continue;
        }

        tokens.push(&source[i..i + 1]);
        i += 1;
    }

    tokens
}

/// Discover groups of equivalent programs (same base name, different extensions).
fn discover_program_groups(dir: &Path) -> BTreeMap<String, BTreeMap<String, std::path::PathBuf>> {
    let mut groups: BTreeMap<String, BTreeMap<String, std::path::PathBuf>> = BTreeMap::new();

    let entries = match fs::read_dir(dir) {
        Ok(e) => e,
        Err(_) => return groups,
    };

    for entry in entries.flatten() {
        let path = entry.path();
        if !path.is_file() { continue; }

        let stem = path.file_stem().and_then(|s| s.to_str()).unwrap_or("").to_string();
        let ext = path.extension().and_then(|s| s.to_str()).unwrap_or("").to_string();

        let lang = match ext.as_str() {
            "knl" => "kernl",
            "py" => "python",
            "rs" => "rust",
            "ts" => "typescript",
            "js" => "javascript",
            "go" => "go",
            _ => continue,
        };

        groups.entry(stem)
            .or_default()
            .insert(lang.to_string(), path);
    }

    groups.retain(|_, files| files.len() >= 2);
    groups
}
