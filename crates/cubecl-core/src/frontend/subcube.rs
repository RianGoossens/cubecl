use super::{CubeContext, CubePrimitive, ExpandElement, UInt};
use crate::prelude::{Bool, ExpandElementTyped};
use crate::{
    ir::{Elem, InitOperator, Item, Operation, Subcube, UnaryOperator},
    unexpanded,
};

/// Returns true if the cube unit has the lowest subcube_unit_id among active unit in the subcube
pub fn subcube_elect() -> Bool {
    unexpanded!()
}

/// Module containing the expand function for [subcube_elect()].
pub mod subcube_elect {

    use super::*;

    /// Expand method of [subcube_elect()].
    pub fn __expand(context: &mut CubeContext) -> ExpandElementTyped<Bool> {
        let output = context.create_local(Item::new(Elem::Bool));
        let out = *output;

        context.register(Operation::Subcube(Subcube::Elect(InitOperator { out })));

        output.into()
    }
}

/// Broadcasts the value from the specified subcube unit at the given index
/// to all active units within that subcube.
#[allow(unused_variables)]
pub fn subcube_broadcast<E: CubePrimitive>(value: E, index: UInt) -> E {
    unexpanded!()
}

/// Module containing the expand function for [subcube_broadcast()].
pub mod subcube_broadcast {

    use super::*;

    /// Expand method of [subcube_broadcast()].
    pub fn __expand<E: CubePrimitive>(
        context: &mut CubeContext,
        value: ExpandElementTyped<E>,
        id: ExpandElementTyped<UInt>,
    ) -> ExpandElementTyped<E> {
        let output = context.create_local(value.expand.item());
        let out = *output;
        let lhs = *value.expand;
        let rhs = *id.expand;

        context.register(Operation::Subcube(Subcube::Broadcast(
            crate::ir::BinaryOperator { lhs, rhs, out },
        )));

        output.into()
    }
}

/// Perform a reduce sum operation across all units in a subcube.
#[allow(unused_variables)]
pub fn subcube_sum<E: CubePrimitive>(value: E) -> E {
    unexpanded!()
}

/// Module containing the expand function for [subcube_sum()].
pub mod subcube_sum {
    use super::*;

    /// Expand method of [subcube_sum()].
    pub fn __expand<E: CubePrimitive>(
        context: &mut CubeContext,
        elem: ExpandElementTyped<E>,
    ) -> ExpandElementTyped<E> {
        let elem: ExpandElement = elem.into();
        let output = context.create_local(elem.item());

        let out = *output;
        let input = *elem;

        context.register(Operation::Subcube(Subcube::Sum(UnaryOperator {
            input,
            out,
        })));

        output.into()
    }
}

/// Perform a reduce prod operation across all units in a subcube.
pub fn subcube_prod<E: CubePrimitive>(_elem: E) -> E {
    unexpanded!()
}

/// Module containing the expand function for [subcube_prod()].
pub mod subcube_prod {
    use super::*;

    /// Expand method of [subcube_prod()].
    pub fn __expand<E: CubePrimitive>(
        context: &mut CubeContext,
        elem: ExpandElementTyped<E>,
    ) -> ExpandElementTyped<E> {
        let elem: ExpandElement = elem.into();
        let output = context.create_local(elem.item());

        let out = *output;
        let input = *elem;

        context.register(Operation::Subcube(Subcube::Prod(UnaryOperator {
            input,
            out,
        })));

        output.into()
    }
}

/// Perform a reduce max operation across all units in a subcube.
pub fn subcube_max<E: CubePrimitive>(_elem: E) -> E {
    unexpanded!()
}

/// Module containing the expand function for [subcube_max()].
pub mod subcube_max {
    use super::*;

    /// Expand method of [subcube_max()].
    pub fn __expand<E: CubePrimitive>(
        context: &mut CubeContext,
        elem: ExpandElementTyped<E>,
    ) -> ExpandElementTyped<E> {
        let elem: ExpandElement = elem.into();
        let output = context.create_local(elem.item());

        let out = *output;
        let input = *elem;

        context.register(Operation::Subcube(Subcube::Max(UnaryOperator {
            input,
            out,
        })));

        output.into()
    }
}

/// Perform a reduce min operation across all units in a subcube.
pub fn subcube_min<E: CubePrimitive>(_elem: E) -> E {
    unexpanded!()
}

/// Module containing the expand function for [subcube_min()].
pub mod subcube_min {
    use super::*;

    /// Expand method of [subcube_min()].
    pub fn __expand<E: CubePrimitive>(
        context: &mut CubeContext,
        elem: ExpandElementTyped<E>,
    ) -> ExpandElementTyped<E> {
        let elem: ExpandElement = elem.into();
        let output = context.create_local(elem.item());

        let out = *output;
        let input = *elem;

        context.register(Operation::Subcube(Subcube::Min(UnaryOperator {
            input,
            out,
        })));

        output.into()
    }
}

/// Perform a reduce all operation across all units in a subcube.
pub fn subcube_all(_elem: Bool) -> Bool {
    unexpanded!()
}

/// Module containing the expand function for [subcube_all()].
pub mod subcube_all {

    use super::*;

    /// Expand method of [subcube_all()].
    pub fn __expand(
        context: &mut CubeContext,
        elem: ExpandElementTyped<Bool>,
    ) -> ExpandElementTyped<Bool> {
        let elem: ExpandElement = elem.into();
        let output = context.create_local(elem.item());

        let out = *output;
        let input = *elem;

        context.register(Operation::Subcube(Subcube::All(UnaryOperator {
            input,
            out,
        })));

        output.into()
    }
}

/// Perform a reduce any operation across all units in a subcube.
pub fn subcube_any(_elem: Bool) -> Bool {
    unexpanded!()
}

/// Module containing the expand function for [subcube_any()].
pub mod subcube_any {

    use super::*;

    /// Expand method of [subcube_any()].
    pub fn __expand(
        context: &mut CubeContext,
        elem: ExpandElementTyped<Bool>,
    ) -> ExpandElementTyped<Bool> {
        let elem: ExpandElement = elem.into();
        let output = context.create_local(elem.item());

        let out = *output;
        let input = *elem;

        context.register(Operation::Subcube(Subcube::Any(UnaryOperator {
            input,
            out,
        })));

        output.into()
    }
}
