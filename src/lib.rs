// #![allow(dead_code)]

pub mod cmd;
pub mod errors;
pub mod k8_manager;
pub mod prelude;
pub mod redis_manager;

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
