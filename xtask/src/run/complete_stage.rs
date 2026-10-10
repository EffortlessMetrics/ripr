//! Alias the shared fixed-only retained-stage owner and observation data.
//! The original controls now live in ripr::process_owner::retained_stage.
//! Generic path allocation stays private and test-only in the shared owner.

pub(crate) use ripr::process_owner::{
    ParentStage, StageBudget, StageDirectoryBinding, StageEntry, StageEntryKind, StageInventory,
    StageInventoryClosure, StageRole, StageRoleBudget, StageRootBinding, StageUsage,
};
