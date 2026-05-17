/// Adds profiling instrumentation calls to LLVM IR.
///
/// For each function definition, inserts:
///   - At entry: `call void @__kernl_profile_enter(i8* <func_name>)`
///   - Before each `ret`: `call void @__kernl_profile_exit(i8* <func_name>)`
///
/// Also adds extern declarations for the profiling runtime functions.
pub fn instrument_llvm_ir(ir: &str) -> String {
    let mut output = String::new();
    let mut declared = false;
    let mut current_func: Option<String> = None;
    let mut string_constants: Vec<(String, String)> = Vec::new();

    for line in ir.lines() {
        let trimmed = line.trim();

        if !declared && (trimmed.starts_with("define ") || trimmed.starts_with("declare ")) {
            output.push_str("declare void @__kernl_profile_enter(i8*)\n");
            output.push_str("declare void @__kernl_profile_exit(i8*)\n");
            output.push_str("declare void @__kernl_profile_report()\n\n");
            declared = true;
        }

        if trimmed.starts_with("define ") {
            if let Some(name) = extract_function_name(trimmed) {
                if !name.starts_with("__kernl_profile") {
                    current_func = Some(name.clone());
                    let const_name = format!("@__kernl_prof_str_{name}");
                    let name_with_null = name.len() + 1;
                    string_constants.push((
                        const_name.clone(),
                        format!(
                            "{const_name} = private unnamed_addr constant [{name_with_null} x i8] c\"{}\\00\"",
                            escape_llvm_string(&name)
                        ),
                    ));

                    output.push_str(line);
                    output.push('\n');

                    if !trimmed.ends_with('}') {
                        output.push_str(&format!(
                            "  call void @__kernl_profile_enter(i8* getelementptr inbounds ([{name_with_null} x i8], [{name_with_null} x i8]* {const_name}, i32 0, i32 0))\n"
                        ));
                    }
                    continue;
                }
            }
        }

        if trimmed.starts_with("ret ") {
            if let Some(ref name) = current_func {
                let const_name = format!("@__kernl_prof_str_{name}");
                let name_with_null = name.len() + 1;
                output.push_str(&format!(
                    "  call void @__kernl_profile_exit(i8* getelementptr inbounds ([{name_with_null} x i8], [{name_with_null} x i8]* {const_name}, i32 0, i32 0))\n"
                ));
            }
        }

        if trimmed == "}" {
            current_func = None;
        }

        output.push_str(line);
        output.push('\n');
    }

    if !string_constants.is_empty() {
        output.push('\n');
        for (_, decl) in &string_constants {
            output.push_str(decl);
            output.push('\n');
        }
    }

    output
}

fn extract_function_name(define_line: &str) -> Option<String> {
    let at_pos = define_line.find('@')?;
    let after_at = &define_line[at_pos + 1..];
    let end = after_at.find('(')?;
    Some(after_at[..end].to_string())
}

fn escape_llvm_string(s: &str) -> String {
    let mut out = String::new();
    for b in s.bytes() {
        if b.is_ascii_alphanumeric() || b == b'_' {
            out.push(b as char);
        } else {
            out.push_str(&format!("\\{b:02X}"));
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn instrument_inserts_enter_exit() {
        let ir = "\
define i64 @add(i64 %a, i64 %b) {
entry:
  %result = add i64 %a, %b
  ret i64 %result
}
";
        let instrumented = instrument_llvm_ir(ir);
        assert!(instrumented.contains("@__kernl_profile_enter"));
        assert!(instrumented.contains("@__kernl_profile_exit"));
        assert!(instrumented.contains("declare void @__kernl_profile_enter(i8*)"));
        assert!(instrumented.contains("declare void @__kernl_profile_exit(i8*)"));
        assert!(instrumented.contains("declare void @__kernl_profile_report()"));
    }

    #[test]
    fn instrument_adds_string_constants() {
        let ir = "\
define i64 @my_func() {
entry:
  ret i64 0
}
";
        let instrumented = instrument_llvm_ir(ir);
        assert!(instrumented.contains("@__kernl_prof_str_my_func"));
        assert!(instrumented.contains("c\"my_func\\00\""));
    }

    #[test]
    fn instrument_skips_profiler_functions() {
        let ir = "\
define void @__kernl_profile_enter(i8* %name) {
entry:
  ret void
}

define i64 @user_fn() {
entry:
  ret i64 42
}
";
        let instrumented = instrument_llvm_ir(ir);
        assert!(instrumented.contains("@__kernl_prof_str_user_fn"));
        assert!(!instrumented.contains("@__kernl_prof_str___kernl_profile_enter"));
    }

    #[test]
    fn instrument_handles_multiple_returns() {
        let ir = "\
define i64 @branch(i64 %x) {
entry:
  %cmp = icmp sgt i64 %x, 0
  br i1 %cmp, label %positive, label %negative

positive:
  ret i64 1

negative:
  ret i64 0
}
";
        let instrumented = instrument_llvm_ir(ir);
        let exit_count = instrumented.matches("@__kernl_profile_exit").count();
        // Two ret statements + the declaration = 3 occurrences
        assert!(exit_count >= 2, "expected at least 2 exit calls, got {exit_count}");
    }

    #[test]
    fn instrument_empty_ir() {
        let instrumented = instrument_llvm_ir("");
        assert!(!instrumented.contains("declare void @__kernl_profile_enter"));
    }
}
