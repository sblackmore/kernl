use crate::parser::ast::{Program, Function, FnMode};

#[derive(Debug)]
pub struct VerifyError {
    pub message: String,
    pub function: String,
}

impl std::fmt::Display for VerifyError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "verify error in '{}': {}", self.function, self.message)
    }
}

impl std::error::Error for VerifyError {}

pub struct Verifier;

impl Verifier {
    pub fn check(program: &Program) -> Vec<VerifyError> {
        let mut errors = Vec::new();

        for item in &program.items {
            if let crate::parser::ast::Item::Function(func) = item {
                errors.extend(Self::check_function(func));
            }
        }

        errors
    }

    fn check_function(func: &Function) -> Vec<VerifyError> {
        let mut errors = Vec::new();

        if func.mode == FnMode::Fluid {
            if func.intent.is_none() {
                errors.push(VerifyError {
                    message: "fluid functions require an intent clause".into(),
                    function: func.name.clone(),
                });
            }
        }

        if func.mode == FnMode::Strict && !func.invariants.is_empty() {
            // Future: statically verify invariants hold for all inputs
        }

        errors
    }
}
