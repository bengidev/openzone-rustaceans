pub mod onboarding;

mod scratch;
#[cfg(test)]
pub(crate) use scratch::ScratchMessage;
pub use scratch::ScratchPanel;

#[cfg(test)]
pub mod dummies;
