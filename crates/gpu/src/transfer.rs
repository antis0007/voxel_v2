//! Shared byte admission for asynchronous uploads competing within one frame.
#[derive(Clone, Copy, Debug)]
pub struct UploadBudget {
    remaining: usize,
}
impl UploadBudget {
    pub fn new(bytes: usize) -> Self {
        Self { remaining: bytes }
    }
    pub fn reserve(&mut self, bytes: usize) -> bool {
        if bytes > self.remaining {
            return false;
        }
        self.remaining -= bytes;
        true
    }
    pub fn remaining(&self) -> usize {
        self.remaining
    }
}
