use crate::instruction::Instruction;

/// A group of instructions sharing the same fence index.
/// Instructions within a FenceGroup have no defined relative execution order
/// (they may run concurrently). All must complete before the next FenceGroup starts.
#[derive(Debug, Clone)]
pub struct FenceGroup {
    /// The fence index (from the ordered list item number).
    pub index: u64,
    /// Instructions within this fence group.
    pub instructions: Vec<Instruction>,
}
