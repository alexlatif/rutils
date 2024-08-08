use error_stack::Context;

/// A generic trace_stack error to use when you don't want to create custom error types.
#[derive(Debug, Default)]
pub struct AnyErr;

impl std::fmt::Display for AnyErr {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "AnyErr")
    }
}

impl Context for AnyErr {}

#[derive(Debug, Default)]
pub struct AnyErr2 {
    context: String,
}

impl AnyErr2 {
    pub fn new(context: impl Into<String>) -> Self {
        AnyErr2 {
            context: context.into(),
        }
    }
}

impl std::fmt::Display for AnyErr2 {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.context)
    }
}

impl Context for AnyErr2 {}
