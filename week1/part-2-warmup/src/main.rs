/* The following exercises were borrowed from Will Crichton's CS 242 Rust lab. */

use std::collections::HashSet;

fn main() {
    println!("Hi! Try running \"cargo test\" to run tests.");
}

#[allow(dead_code)]
fn add_n(v: Vec<i32>, n: i32) -> Vec<i32> {
    let mut new_v = Vec::new();
    for i in v {
        new_v.push(i + n);
    }
    new_v
}

#[allow(dead_code)]
fn add_n_inplace(v: &mut Vec<i32>, n: i32) {
    for i in 0..v.len() {
        v[i] += n;
    }
}

#[allow(dead_code)]
fn dedup(v: &mut Vec<i32>) {
    let mut set = HashSet::new();
    let mut new_v = Vec::new();
    for i in v.clone() {
        if set.insert(i) {
            new_v.push(i);
        }
    }

    *v = new_v;
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn test_add_n() {
        assert_eq!(add_n(vec![1], 2), vec![3]);
    }

    #[test]
    fn test_add_n_inplace() {
        let mut v = vec![1];
        add_n_inplace(&mut v, 2);
        assert_eq!(v, vec![3]);
    }

    #[test]
    fn test_dedup() {
        let mut v = vec![3, 1, 0, 1, 4, 4];
        dedup(&mut v);
        assert_eq!(v, vec![3, 1, 0, 4]);
    }
}
