//! Native acceptance needs a successful playable presentation AND a writer ack.
use std::time::{Duration, Instant};
#[derive(Default)]
pub(super) struct Completion { requested: Option<Instant>, presented: bool, rejected: Option<Failure> }
#[derive(Debug, PartialEq, Eq)]
pub(super) enum Outcome { Pending, Passed, Failed(Failure) }
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Failure { LoadingFrame, Presentation, CaptureWrite, CaptureTimeout }
impl Failure {
    pub fn label(self) -> &'static str {
        match self {
            Self::LoadingFrame => "loading_frame",
            Self::Presentation => "presentation_failed",
            Self::CaptureWrite => "capture_write_failed",
            Self::CaptureTimeout => "capture_timeout",
        }
    }
}
impl Completion {
    pub fn arm(&mut self, now: Instant) -> bool {
        if self.requested.is_some() { return false; }
        self.requested = Some(now);
        true
    }
    pub fn presented(&self) -> bool { self.presented }
    pub fn observe(&mut self, now: Instant, playable: bool, rendered: bool, saved: Option<bool>) -> Outcome {
        let Some(started) = self.requested else { return Outcome::Pending; };
        // The first observation is for the frame in which the capture was armed.
        if !self.presented && self.rejected.is_none() {
            self.presented = playable && rendered;
            self.rejected = if !playable { Some(Failure::LoadingFrame) } else if !rendered { Some(Failure::Presentation) } else { None };
        }
        if let Some(reason) = self.rejected { return Outcome::Failed(reason); }
        if saved == Some(false) { return Outcome::Failed(Failure::CaptureWrite); }
        if self.presented && saved == Some(true) { return Outcome::Passed; }
        if now.saturating_duration_since(started) >= Duration::from_secs(30) { return Outcome::Failed(Failure::CaptureTimeout); }
        Outcome::Pending
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn cannot_finish_before_reload_or_without_saved_capture() {
        let now=Instant::now(); let mut c=Completion::default();
        assert_eq!(c.observe(now+Duration::from_secs(300),true,true,Some(true)),Outcome::Pending);
        assert!(c.arm(now)); assert!(!c.arm(now));
        assert_eq!(c.observe(now,true,true,None),Outcome::Pending);
        assert_eq!(c.observe(now+Duration::from_secs(1),true,true,Some(true)),Outcome::Passed);
    }
    #[test]
    fn loading_or_failed_presentation_cannot_pass_even_with_a_png() {
        for (playable, rendered) in [(false,true),(true,false)] {
            let now=Instant::now(); let mut c=Completion::default(); c.arm(now);
            assert_eq!(c.observe(now,playable,rendered,Some(true)),Outcome::Failed(if playable { Failure::Presentation } else { Failure::LoadingFrame }));
        }
    }
    #[test]
    fn missing_or_failed_capture_terminates_without_pass() {
        let now=Instant::now(); let mut c=Completion::default(); c.arm(now);
        assert_eq!(c.observe(now,true,true,Some(false)),Outcome::Failed(Failure::CaptureWrite));
        let mut c=Completion::default();c.arm(now);
        assert_eq!(c.observe(now,true,true,None),Outcome::Pending);
        assert_eq!(c.observe(now+Duration::from_secs(30),true,true,None),Outcome::Failed(Failure::CaptureTimeout));
    }
}
