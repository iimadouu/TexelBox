#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BuildPlan {
    Free,
    Trial,
    Pro,
}
impl BuildPlan {
    pub fn detect() -> Self {
        Self::Free
    }
    pub const fn is_free(self) -> bool {
        matches!(self, Self::Free)
    }
    pub const fn is_trial(self) -> bool {
        matches!(self, Self::Trial)
    }
    pub const fn is_pro(self) -> bool {
        matches!(self, Self::Pro)
    }
}
impl Default for BuildPlan {
    fn default() -> Self {
        Self::Free
    }
}
