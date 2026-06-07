pub mod cost;
pub mod health;
pub mod logs;
pub mod plugins;
pub mod queue;
pub mod subjects;
pub mod workflows;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ViewId {
    Workflows = 0,
    Subjects = 1,
    Queue = 2,
    Health = 3,
    Logs = 4,
    Cost = 5,
    Plugins = 6,
}

impl ViewId {
    pub const ALL: [ViewId; 7] = [
        ViewId::Workflows,
        ViewId::Subjects,
        ViewId::Queue,
        ViewId::Health,
        ViewId::Logs,
        ViewId::Cost,
        ViewId::Plugins,
    ];

    pub fn from_idx(idx: usize) -> Self {
        Self::ALL[idx.min(6)]
    }

    pub fn idx(self) -> usize {
        self as usize
    }
}
