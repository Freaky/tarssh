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

#[cfg(test)]
quickcheck::quickcheck! {
    fn prop_retain_unordered(test: Vec<u32>, cutoff: u32) -> bool {
        let mut expected = test.clone();
        expected.retain(|i| *i < cutoff);
        expected.sort_unstable();

        let mut test = test;
        test.retain_unordered(|i| *i < cutoff);
        test.sort_unstable();

        test == expected
    }
}
