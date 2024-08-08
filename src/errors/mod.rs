mod any;
mod macros;

pub use any::{AnyErr, AnyErr2};

/// Shorthand for a [`Result`] with a [`error_stack::Report`] as the error variant
pub type RResult<T, C> = Result<T, error_stack::Report<C>>;

/// Easily import all useful error items. Useful to put inside a crate prelude.
pub mod prelude {
    #[allow(unused_imports)]
    pub use error_stack::{Report, ResultExt};

    #[allow(unused_imports)]
    pub use super::{AnyErr, AnyErr2, RResult};

    #[allow(unused_imports)]
    pub use crate::err2;
    #[allow(unused_imports)]
    pub use crate::{anyerr, err, panic_on_err, panic_on_err_async};
}

#[cfg(test)]
mod tests {
    use super::*;
    pub use crate::err2;
    use error_stack::{Report, ResultExt};

    // A simple function that always returns an error
    fn error_prone_function() -> RResult<(), AnyErr2> {
        Err(Report::new(err2!("An error occurred")))
    }

    // A wrapper function that calls the error-prone function and adds more context
    fn wrapper_function() -> RResult<(), AnyErr2> {
        error_prone_function().change_context(err2!("Wrapper function context"))
    }

    #[test]
    fn test_error_propagation() {
        let result = wrapper_function();

        match result {
            Err(report) => {
                let details = format!("{:?}", report);
                assert!(details.contains("Wrapper function context"));
                assert!(details.contains("An error occurred"));
                println!("{}", details);
            }
            Ok(_) => panic!("The test should have failed with an error"),
        }
    }
}
