use serde::{Deserialize, Serialize};
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Plan {
    Free,
    Pro,
    Trial,
}
impl Plan {
    pub fn satisfies(self, required: Plan) -> bool {
        matches!(
            (self, required),
            (Plan::Pro, _) | (Plan::Trial, _) | (Plan::Free, Plan::Free)
        )
    }
    pub fn is_paid(self) -> bool {
        matches!(self, Plan::Pro | Plan::Trial)
    }
}
impl Default for Plan {
    fn default() -> Self {
        Plan::Free
    }
}
