/// Built-in functions available in every kernl program.
///
/// Uses string-based type representations so the stdlib can be defined
/// independently of the resolved `Ty` system — the type checker maps
/// these strings to concrete types at check time.

/// A single parameter of a built-in function.
#[derive(Debug, Clone)]
pub struct BuiltinParam {
    pub name: &'static str,
    pub ty: &'static str,
}

/// Descriptor for one built-in function.
#[derive(Debug, Clone)]
pub struct Builtin {
    pub name: &'static str,
    pub params: &'static [BuiltinParam],
    pub return_ty: &'static str,
    pub description: &'static str,
}

const FILTER: Builtin = Builtin {
    name: "filter",
    params: &[
        BuiltinParam { name: "list", ty: "[T]" },
        BuiltinParam { name: "predicate", ty: "(T -> bool)" },
    ],
    return_ty: "[T]",
    description: "Filter list by predicate",
};

const REDUCE: Builtin = Builtin {
    name: "reduce",
    params: &[
        BuiltinParam { name: "list", ty: "[T]" },
        BuiltinParam { name: "op", ty: "(T, T -> T)" },
    ],
    return_ty: "T",
    description: "Reduce list with binary operator",
};

const MAP: Builtin = Builtin {
    name: "map",
    params: &[
        BuiltinParam { name: "list", ty: "[T]" },
        BuiltinParam { name: "fn", ty: "(T -> U)" },
    ],
    return_ty: "[U]",
    description: "Transform each element",
};

const MAX_INT: Builtin = Builtin {
    name: "max",
    params: &[
        BuiltinParam { name: "a", ty: "int" },
        BuiltinParam { name: "b", ty: "int" },
    ],
    return_ty: "int",
    description: "Larger of two integers",
};

const MAX_FLOAT: Builtin = Builtin {
    name: "max",
    params: &[
        BuiltinParam { name: "a", ty: "float" },
        BuiltinParam { name: "b", ty: "float" },
    ],
    return_ty: "float",
    description: "Larger of two floats",
};

const MIN_INT: Builtin = Builtin {
    name: "min",
    params: &[
        BuiltinParam { name: "a", ty: "int" },
        BuiltinParam { name: "b", ty: "int" },
    ],
    return_ty: "int",
    description: "Smaller of two integers",
};

const MIN_FLOAT: Builtin = Builtin {
    name: "min",
    params: &[
        BuiltinParam { name: "a", ty: "float" },
        BuiltinParam { name: "b", ty: "float" },
    ],
    return_ty: "float",
    description: "Smaller of two floats",
};

const LEN: Builtin = Builtin {
    name: "len",
    params: &[BuiltinParam { name: "list", ty: "[T]" }],
    return_ty: "int",
    description: "List length",
};

const PRINT: Builtin = Builtin {
    name: "print",
    params: &[BuiltinParam { name: "value", ty: "T" }],
    return_ty: "void",
    description: "Output a value",
};

const ABS: Builtin = Builtin {
    name: "abs",
    params: &[BuiltinParam { name: "x", ty: "int" }],
    return_ty: "int",
    description: "Absolute value",
};

const SQRT: Builtin = Builtin {
    name: "sqrt",
    params: &[BuiltinParam { name: "x", ty: "float" }],
    return_ty: "float",
    description: "Square root",
};

const CONCAT: Builtin = Builtin {
    name: "concat",
    params: &[
        BuiltinParam { name: "a", ty: "str" },
        BuiltinParam { name: "b", ty: "str" },
    ],
    return_ty: "str",
    description: "String concatenation",
};

const RANGE: Builtin = Builtin {
    name: "range",
    params: &[
        BuiltinParam { name: "start", ty: "int" },
        BuiltinParam { name: "end", ty: "int" },
    ],
    return_ty: "[int]",
    description: "Generate integer range",
};

static BUILTINS: &[Builtin] = &[
    FILTER, REDUCE, MAP,
    MAX_INT, MAX_FLOAT,
    MIN_INT, MIN_FLOAT,
    LEN, PRINT, ABS, SQRT,
    CONCAT, RANGE,
];

/// All built-in function definitions (includes overload variants).
pub fn builtins() -> &'static [Builtin] {
    BUILTINS
}

/// Unique builtin names (overloads collapsed).
pub fn builtin_names() -> Vec<&'static str> {
    let mut names: Vec<&str> = BUILTINS.iter().map(|b| b.name).collect();
    names.sort_unstable();
    names.dedup();
    names
}

/// `true` if `name` refers to a built-in function.
pub fn is_builtin(name: &str) -> bool {
    BUILTINS.iter().any(|b| b.name == name)
}

/// Return the first matching builtin (for overloaded builtins, returns the
/// first variant — callers needing overload resolution should iterate
/// `builtins()` directly).
pub fn get_builtin(name: &str) -> Option<&'static Builtin> {
    BUILTINS.iter().find(|b| b.name == name)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn all_builtins_registered() {
        let expected = [
            "abs", "concat", "filter", "len", "map", "max",
            "min", "print", "range", "reduce", "sqrt",
        ];
        let names = builtin_names();
        for name in &expected {
            assert!(names.contains(name), "missing builtin: {name}");
        }
    }

    #[test]
    fn lookup_existing() {
        let b = get_builtin("filter").expect("filter should exist");
        assert_eq!(b.name, "filter");
        assert_eq!(b.params.len(), 2);
        assert_eq!(b.return_ty, "[T]");
    }

    #[test]
    fn lookup_missing() {
        assert!(get_builtin("nonexistent").is_none());
    }

    #[test]
    fn is_builtin_positive() {
        assert!(is_builtin("max"));
        assert!(is_builtin("print"));
        assert!(is_builtin("sqrt"));
    }

    #[test]
    fn is_builtin_negative() {
        assert!(!is_builtin("foo"));
        assert!(!is_builtin(""));
    }

    #[test]
    fn overloaded_builtins_present() {
        let max_variants: Vec<_> = builtins().iter().filter(|b| b.name == "max").collect();
        assert_eq!(max_variants.len(), 2, "max should have int and float variants");

        let min_variants: Vec<_> = builtins().iter().filter(|b| b.name == "min").collect();
        assert_eq!(min_variants.len(), 2, "min should have int and float variants");
    }

    #[test]
    fn builtin_descriptions_non_empty() {
        for b in builtins() {
            assert!(!b.description.is_empty(), "{} has empty description", b.name);
        }
    }
}
