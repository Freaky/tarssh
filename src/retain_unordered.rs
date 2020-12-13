/// Trait that provides a `retain_unordered` method.
pub trait RetainUnordered<T> {
    /// Retains only the elements for which the predicate returns true, without
    /// any guarantees over visit or final order.
    fn retain_unordered<F>(&mut self, f: F)
    where
        F: FnMut(&mut T) -> bool;
}

impl<T> RetainUnordered<T> for Vec<T> {
    fn retain_unordered<F>(&mut self, mut f: F)
    where
        F: FnMut(&mut T) -> bool,
    {
        let mut i = 0;

        while i < self.len() {
            if f(&mut self[i]) {
                i += 1;
            } else if self.len() > 1 {
                self.swap_remove(i);
            } else {
                self.remove(i);
            }
        }
    }
}

#[test]
fn test_vec_retain_unordered() {
    let tests = vec![
        (vec![0, 1, 2, 3, 4, 5], vec![0, 2, 4]),
        (vec![0, 2, 4, 6, 8], vec![0, 2, 4, 6, 8]),
        (vec![1, 3, 5, 7, 9], vec![]),
        (vec![0, 2], vec![0, 2]),
        (vec![1, 3], vec![]),
        (vec![0], vec![0]),
        (vec![1], vec![]),
        (vec![], vec![]),
    ];

    for (mut t, exp) in tests.into_iter() {
        t.retain_unordered(|i| *i % 2 == 0);
        t.sort_unstable();
        assert_eq!(t, exp);
    }
}
