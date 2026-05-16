/// Generates LLVM IR debug metadata (DWARF).
/// When enabled, the emitted IR includes !dbg attachments and
/// DICompileUnit/DIFile/DISubprogram/DILocation metadata.

pub struct DebugInfoEmitter {
    file_name: String,
    directory: String,
    metadata_id: usize,
    metadata: Vec<String>,
}

impl DebugInfoEmitter {
    pub fn new(file_name: &str, directory: &str) -> Self {
        Self {
            file_name: file_name.to_string(),
            directory: directory.to_string(),
            metadata_id: 0,
            metadata: Vec::new(),
        }
    }

    fn next_id(&mut self) -> usize {
        let id = self.metadata_id;
        self.metadata_id += 1;
        id
    }

    /// Emit the compile unit metadata (must be called first).
    /// Returns the compile-unit metadata ID.
    pub fn emit_compile_unit(&mut self) -> usize {
        let cu_id = self.next_id();
        let file_id = self.next_id();

        self.metadata.push(format!(
            "!{file_id} = !DIFile(filename: \"{}\", directory: \"{}\")",
            self.file_name, self.directory
        ));

        self.metadata.push(format!(
            "!{cu_id} = distinct !DICompileUnit(language: DW_LANG_C, file: !{file_id}, \
             producer: \"kernlc {}\", isOptimized: false, emissionKind: FullDebug)",
            env!("CARGO_PKG_VERSION")
        ));

        cu_id
    }

    /// Emit a subprogram (function) debug entry.
    /// Returns the subprogram metadata ID.
    pub fn emit_subprogram(&mut self, func_name: &str, line: usize, file_id: usize) -> usize {
        let sp_id = self.next_id();
        let type_id = self.next_id();

        self.metadata.push(format!(
            "!{type_id} = !DISubroutineType(types: !{{}})"
        ));

        self.metadata.push(format!(
            "!{sp_id} = distinct !DISubprogram(name: \"{func_name}\", scope: !{file_id}, \
             file: !{file_id}, line: {line}, type: !{type_id}, isLocal: false, \
             isDefinition: true, scopeLine: {line}, unit: !0)"
        ));

        sp_id
    }

    /// Emit a debug location (line + column).
    /// Returns the location metadata ID.
    pub fn emit_location(&mut self, line: usize, col: usize, scope_id: usize) -> usize {
        let loc_id = self.next_id();
        self.metadata.push(format!(
            "!{loc_id} = !DILocation(line: {line}, column: {col}, scope: !{scope_id})"
        ));
        loc_id
    }

    /// The file metadata ID is always 1 (allocated second after compile unit at 0).
    pub fn file_id(&self) -> usize {
        1
    }

    /// Get all metadata as LLVM IR text to append to the module.
    pub fn finish(&self) -> String {
        let mut out = String::new();
        out.push_str("\n; Debug metadata\n");
        out.push_str("!llvm.dbg.cu = !{!0}\n");
        out.push_str("!llvm.module.flags = !{!100, !101}\n");
        out.push_str("!100 = !{i32 2, !\"Dwarf Version\", i32 4}\n");
        out.push_str("!101 = !{i32 2, !\"Debug Info Version\", i32 3}\n\n");
        for m in &self.metadata {
            out.push_str(m);
            out.push('\n');
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn compile_unit_contains_dicompileunit() {
        let mut emitter = DebugInfoEmitter::new("test.knl", "/tmp");
        emitter.emit_compile_unit();
        let output = emitter.finish();
        assert!(output.contains("!DICompileUnit"));
    }

    #[test]
    fn compile_unit_contains_difile() {
        let mut emitter = DebugInfoEmitter::new("test.knl", "/tmp");
        emitter.emit_compile_unit();
        let output = emitter.finish();
        assert!(output.contains("!DIFile(filename: \"test.knl\", directory: \"/tmp\")"));
    }

    #[test]
    fn subprogram_emits_disubprogram() {
        let mut emitter = DebugInfoEmitter::new("test.knl", "/tmp");
        emitter.emit_compile_unit();
        let sp_id = emitter.emit_subprogram("my_func", 5, emitter.file_id());
        let output = emitter.finish();
        assert!(output.contains("!DISubprogram(name: \"my_func\""));
        assert!(output.contains(&format!("!{sp_id} = distinct !DISubprogram")));
    }

    #[test]
    fn location_emits_dilocation() {
        let mut emitter = DebugInfoEmitter::new("test.knl", "/tmp");
        emitter.emit_compile_unit();
        let sp_id = emitter.emit_subprogram("f", 1, emitter.file_id());
        let loc_id = emitter.emit_location(10, 5, sp_id);
        let output = emitter.finish();
        assert!(output.contains(&format!(
            "!{loc_id} = !DILocation(line: 10, column: 5, scope: !{sp_id})"
        )));
    }

    #[test]
    fn metadata_ids_are_sequential() {
        let mut emitter = DebugInfoEmitter::new("test.knl", "/tmp");
        let cu_id = emitter.emit_compile_unit();
        assert_eq!(cu_id, 0);
        let sp_id = emitter.emit_subprogram("f", 1, emitter.file_id());
        assert_eq!(sp_id, 2);
        let loc_id = emitter.emit_location(1, 1, sp_id);
        assert_eq!(loc_id, 4);
    }

    #[test]
    fn finish_includes_module_flags() {
        let mut emitter = DebugInfoEmitter::new("test.knl", "/tmp");
        emitter.emit_compile_unit();
        let output = emitter.finish();
        assert!(output.contains("!llvm.dbg.cu = !{!0}"));
        assert!(output.contains("Dwarf Version"));
        assert!(output.contains("Debug Info Version"));
    }
}
