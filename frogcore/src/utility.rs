use std::{cmp::Ordering, collections::BinaryHeap};

pub(crate) fn n_min<T>(list: &[T], num: usize) -> Vec<usize>
where
    T: Copy,
    f64: From<T>,
{
    let mut heap = BinaryHeap::with_capacity(list.len());
    list.iter()
        .enumerate()
        .for_each(|(n, x)| heap.push(NonNanRev((*x).into(), n)));
    let out_size = if num <= list.len() { num } else { list.len() };
    (0..out_size).map(|_| heap.pop().unwrap().1).collect()
}
struct NonNanRev(f64, usize);

impl PartialEq for NonNanRev {
    fn eq(&self, other: &Self) -> bool {
        if self.0.is_nan() || other.0.is_nan() {
            panic!("Should not be NaN");
        }

        self == other
    }
}
impl Eq for NonNanRev {}

impl PartialOrd for NonNanRev {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        other.0.partial_cmp(&self.0)
    }
}

impl Ord for NonNanRev {
    fn cmp(&self, other: &NonNanRev) -> Ordering {
        self.partial_cmp(other).expect("Should not be NaN")
    }
}

