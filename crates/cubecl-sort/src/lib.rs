use cubecl::{
    cube,
    prelude::{Array, Int, Numeric},
};

/// The trait for comparative sorting algorithms.
#[cube]
pub trait Sort: Send + Sync + 'static {
    fn sort<N: Numeric>(input: &mut Array<N>);
}

/// Some sorting algorithms only work on integers and not on floats.
#[cube]
pub trait IntSort: Send + Sync + 'static {
    fn sort_ints<I: Int>(input: &mut Array<I>);
}

/// Every comparative sorting algorithm is automatically also an IntSort
#[cube]
impl<S: Sort> IntSort for S {
    fn sort_ints<I: Int>(input: &mut Array<I>) {
        Self::sort(input);
    }
}
