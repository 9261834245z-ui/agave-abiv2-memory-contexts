//! ABIv2 memory-context research module.

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PermissionState {
    ReadOnly,
    Writable,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AccountRegion {
    pub writable: bool,
}

impl Default for AccountRegion {
    fn default() -> Self {
        Self { writable: false }
    }
}

#[derive(Debug, Default)]
pub struct FrameWritableSnapshot {
    pub entries: Vec<(usize, bool)>,
}

#[derive(Debug, Default)]
pub struct MemoryContexts {
    pub account_regions: Vec<AccountRegion>,
    pub snapshots: Vec<FrameWritableSnapshot>,
}

impl MemoryContexts {
    pub fn push_placeholder(&mut self) {
        self.snapshots.push(FrameWritableSnapshot::default());
    }

    pub fn pop(&mut self) {
        if let Some(snapshot) = self.snapshots.pop() {
            if self.snapshots.is_empty() {
                return;
            }

            for (index, previous_writable) in snapshot.entries {
                if let Some(region) = self.account_regions.get_mut(index) {
                    region.writable = previous_writable;
                }
            }
        }
    }

    pub fn update_account_permissions(&mut self, indexes: &[(usize, bool)]) {
        if self.snapshots.is_empty() {
            self.snapshots.push(FrameWritableSnapshot::default());
        }

        let snapshot = self.snapshots.last_mut().unwrap();
        snapshot.entries.clear();

        for (index, writable) in indexes {
            if self.account_regions.len() <= *index {
                self.account_regions.resize_with(*index + 1, Default::default);
            }

            if let Some(region) = self.account_regions.get_mut(*index) {
                let previous_value = region.writable;
                snapshot.entries.push((*index, previous_value));
                region.writable = *writable;
            }
        }
    }

    pub fn region(&self, index: usize) -> Option<PermissionState> {
        self.account_regions.get(index).map(|region| {
            if region.writable {
                PermissionState::Writable
            } else {
                PermissionState::ReadOnly
            }
        })
    }
}
