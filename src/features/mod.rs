pub mod onboarding;

mod scratch;
pub use scratch::ScratchPanel;
#[cfg(test)]
pub(crate) use scratch::ScratchMessage;

#[cfg(test)]
pub mod dummies;
