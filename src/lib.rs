#![allow(dead_code)]

pub mod cmd;
pub mod endpoints;
pub mod errors;
// pub mod logger;
pub mod files;
pub mod prelude;
pub mod python;
pub mod redis_manager;
pub mod redis_tracing;
pub mod testing;

// pub mod k8_manager;
pub fn add(left: usize, right: usize) -> usize {
    left + right
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn it_works() {
        let result = add(2, 2);
        assert_eq!(result, 4);
    }
}
