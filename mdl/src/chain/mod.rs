pub mod fence_group;

use crate::chain::fence_group::FenceGroup;

/// An ordered sequence of fence groups representing a block's execution plan.
/// FenceGroups execute in order: all instructions in group N complete before
/// any instruction in group N+1 begins.
#[derive(Debug, Clone)]
pub struct Chain {
    pub groups: Vec<FenceGroup>,
}

impl Chain {
    pub fn empty() -> Self {
        Chain { groups: Vec::new() }
    }

    pub fn is_empty(&self) -> bool {
        self.groups.is_empty()
    }
}
