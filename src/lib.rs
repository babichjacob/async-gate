use std::{ops::Not, str::FromStr};

use thiserror::Error;
use tokio::sync::watch;

#[derive(Debug, PartialEq, Eq, Clone, Copy, Hash)]
pub enum Gateway {
    Raised,
    Lowered,
}
pub use Gateway::{Lowered, Raised};

impl Not for Gateway {
    type Output = Gateway;

    fn not(self) -> Self::Output {
        match self {
            Raised => Lowered,
            Lowered => Raised,
        }
    }
}

impl std::fmt::Display for Gateway {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{}",
            match self {
                Raised => "Raised",
                Lowered => "Lowered",
            }
        )
    }
}

#[derive(Debug, Error)]
#[error("failed to parse Gateway: provided string was not `Raised` or `Lowered`")]
pub struct ParseGatewayError;

impl FromStr for Gateway {
    type Err = ParseGatewayError;

    #[inline]
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "Raised" => Ok(Raised),
            "Lowered" => Ok(Lowered),
            _ => Err(ParseGatewayError),
        }
    }
}

/// The gate was dropped, but we still know what value it had before dropping
#[derive(Debug, Error)]
#[error("gate was {0} before dropping")]
pub struct BeforeGateDropped(pub Gateway);

/// The gate was dropped, so raising or lowering it achieves nothing
#[derive(Debug, Error)]
#[error("gate was dropped")]
pub struct GateDropped;

/// The lever was dropped while the gate was raised, so the gate will never be lowered again
#[derive(Debug, Error)]
#[error("lever was dropped while raised")]
pub struct LeverDroppedWhileRaised;

/// The lever was dropped while the gate was raised, so the gate will never be lowered again
#[derive(Debug, Error)]
#[error("lever was dropped while lowered")]
pub struct LeverDroppedWhileLowered;

/// A lever that can [`raise`] and [`lower`] the gate it's associated with
///
/// [`raise`]: Lever::raise
/// [`lower`]: Lever::lower
pub struct Lever {
    sender: watch::Sender<Gateway>,
}

impl Lever {
    /// Raise the gate.
    /// This wakes all tasks waiting on [`Gate::raised`]
    /// (and later calls to it will resolve immediately until the gate is [`lower`]ed).
    /// # Errors
    /// If the gate was dropped, an `Err` is returned.
    ///
    /// [`lower`]: Lever::lower
    pub fn raise(&self) -> Result<(), GateDropped> {
        if self.gate_was_dropped() {
            Err(GateDropped)
        } else {
            self.sender.send_if_modified(|gateway| match gateway {
                Raised => false,
                Lowered => {
                    *gateway = Raised;
                    true
                }
            });

            Ok(())
        }
    }

    /// Lower the gate.
    /// This wakes all tasks waiting on [`Gate::lowered`]
    /// (and later calls to it will resolve immediately until the gate is [`raise`]d).
    /// # Errors
    /// If the gate was dropped, an `Err` is returned.
    ///
    /// [`raise`]: Lever::raise
    pub fn lower(&self) -> Result<(), GateDropped> {
        if self.gate_was_dropped() {
            Err(GateDropped)
        } else {
            self.sender.send_if_modified(|gateway| match gateway {
                Lowered => false,
                Raised => {
                    *gateway = Lowered;
                    true
                }
            });

            Ok(())
        }
    }

    /// Returns `Ok(true)` if the gate is raised and `Ok(false)` if it's lowered,
    /// # Errors
    /// If the gate was dropped and was raised before dropping, an `Err(BeforeGateDropped(Raised))` is returned.
    /// Likewise, if the gate was dropped and was lowered before dropping, an `Err(BeforeGateDropped(Lowered))` is returned.
    pub fn is_raised(&self) -> Result<bool, BeforeGateDropped> {
        let gateway = self.sender.borrow();

        if self.gate_was_dropped() {
            Err(BeforeGateDropped(*gateway))
        } else {
            let is_raised = matches!(*gateway, Raised);
            Ok(is_raised)
        }
    }

    /// Returns `Ok(true)` if the gate is lowered and `Ok(false)` if it's raised.
    /// # Errors
    /// If the gate was dropped and was lowered before dropping, an `Err(BeforeGateDropped(Lowered))` is returned.
    /// Likewise, if the gate was dropped and was raised before dropping, an `Err(BeforeGateDropped(Raised))` .
    pub fn is_lowered(&self) -> Result<bool, BeforeGateDropped> {
        let gateway = self.sender.borrow();

        if self.gate_was_dropped() {
            Err(BeforeGateDropped(*gateway))
        } else {
            let is_lowered = matches!(*gateway, Lowered);
            Ok(is_lowered)
        }
    }

    /// Returns `true` if the gate associated with this lever has been dropped
    /// and `false` if it hasn't.
    #[must_use]
    pub fn gate_was_dropped(&self) -> bool {
        self.sender.is_closed()
    }
}

/// A gate that can be checked if [`is_raised`] or [`is_lowered`] immediately,
/// or can be waited on to be [`raised`] or [`lowered`].
///
/// [`is_raised`]: Gate::is_raised
/// [`is_lowered`]: Gate::is_lowered
/// [`raised`]: Gate::raised
/// [`lowered`]: Gate::lowered
#[derive(Clone)]
pub struct Gate {
    receiver: watch::Receiver<Gateway>,
}

impl Gate {
    /// Returns true if the gate (even if the lever has been dropped) is raised and false if it's lowered.
    #[must_use]
    pub fn is_raised(&self) -> bool {
        matches!(*self.receiver.borrow(), Raised)
    }

    /// Returns true if the gate (even if the lever has been dropped) is lowered and false if it's raised.
    #[must_use]
    pub fn is_lowered(&self) -> bool {
        matches!(*self.receiver.borrow(), Lowered)
    }

    /// Wait until the gate is raised
    /// (by a call to [`Lever::raise`])
    /// # Errors
    /// If the lever is dropped while the gate is lowered, an `Err` is returned.
    pub async fn raised(&mut self) -> Result<(), LeverDroppedWhileLowered> {
        match self
            .receiver
            .wait_for(|gateway| matches!(*gateway, Raised))
            .await
        {
            Ok(_) => Ok(()),
            Err(_) => Err(LeverDroppedWhileLowered),
        }
    }

    /// Wait until the gate is lowered
    /// (by a call to [`Lever::lower`])
    /// # Errors
    /// If the lever is dropped while the gate is raised, an `Err` is returned.
    pub async fn lowered(&mut self) -> Result<(), LeverDroppedWhileRaised> {
        match self
            .receiver
            .wait_for(|gateway| matches!(*gateway, Lowered))
            .await
        {
            Ok(_) => Ok(()),
            Err(_) => Err(LeverDroppedWhileRaised),
        }
    }

    /// Returns `true` if the lever associated with this gate has been dropped
    /// and `false` if it hasn't.
    #[must_use]
    pub fn lever_was_dropped(&self) -> bool {
        self.receiver.has_changed().is_err()
    }
}

/// Create a [`Gate`] in the given `initial` state.
/// The [`Lever`] that it is returned with can raise and lower the gate.
#[must_use]
#[inline]
pub fn new(initial: Gateway) -> (Lever, Gate) {
    let (sender, receiver) = watch::channel(initial);

    let lever = Lever { sender };
    let gate = Gate { receiver };

    (lever, gate)
}

/// Create a [`Gate`] that is initially raised.
/// The [`Lever`] that it is returned with can raise and lower the gate.
#[must_use]
#[inline]
pub fn new_raised() -> (Lever, Gate) {
    new(Raised)
}

/// Create a [`Gate`] that is initially lowered.
/// The [`Lever`] that it is returned with can raise and lower the gate.
#[must_use]
#[inline]
pub fn new_lowered() -> (Lever, Gate) {
    new(Lowered)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Tests that the `new_raised` function returns
    /// a `Gate` that is initially raised,
    /// just like the name / docs claim it does.
    #[test]
    fn starts_raised_like_it_claims() {
        let (lever, gate) = new_raised();

        assert!(gate.is_raised());
        assert!(!gate.is_lowered());

        assert!(lever.is_raised().unwrap());
        assert!(!lever.is_lowered().unwrap());
    }

    /// Tests that the `new_lowered` function returns
    /// a `Gate` that is initially lowered,
    /// just like the name / docs claim it does.
    #[test]
    fn starts_lowered_like_it_claims() {
        let (lever, gate) = new_lowered();

        assert!(gate.is_lowered());
        assert!(!gate.is_raised());

        assert!(lever.is_lowered().unwrap());
        assert!(!lever.is_raised().unwrap());
    }

    /// Tests that `raised` and `lowered` resolve instantly
    /// when the `Gate` is already `Raised` or `Lowered` respectively.
    #[test]
    fn resolves_instantly() {
        let (_raised_lever, mut raised_gate) = new_raised();
        let (_lowered_lever, mut lowered_gate) = new_lowered();

        tokio_test::assert_ready!(tokio_test::task::spawn(raised_gate.raised()).poll()).unwrap();
        tokio_test::assert_ready!(tokio_test::task::spawn(lowered_gate.lowered()).poll()).unwrap();
    }

    /// Tests that `lowered` and `raised` do not resolve instantly
    /// when the `Gate` is currently `Raised` or `Lowered` respectively.
    ///
    /// Then, it tests that they do indeed once it has been `lower`ed or `raise`d respectively.
    #[test]
    fn does_not_resolve_until_satisfied() {
        let (initially_raised_lever, mut initially_raised_gate) = new_raised();
        let (initially_lowered_lever, mut initially_lowered_gate) = new_lowered();

        let mut became_lowered = tokio_test::task::spawn(initially_raised_gate.lowered());
        let mut became_raised = tokio_test::task::spawn(initially_lowered_gate.raised());

        tokio_test::assert_pending!(became_lowered.poll());
        tokio_test::assert_pending!(became_raised.poll());

        initially_raised_lever.lower().unwrap();
        initially_lowered_lever.raise().unwrap();

        tokio_test::assert_ready_ok!(became_lowered.poll());
        tokio_test::assert_ready_ok!(became_raised.poll());
    }

    /// Tests that calling `raised` on a `Gate` that was `Lowered` when its `Lever` dropped results in an `Err`.
    #[test]
    fn lowered_gate_gives_err_on_raised_when_lever_dropped() {
        let (lever, mut gate) = new_lowered();

        drop(lever);

        assert!(gate.lever_was_dropped());

        tokio_test::assert_ready_err!(tokio_test::task::spawn(gate.raised()).poll());
    }

    /// Tests that calling `lowered` on a `Gate` that was `Raised` when its `Lever` dropped results in an `Err`.
    #[test]
    fn raised_gate_gives_err_on_lowered_when_lever_dropped() {
        let (lever, mut gate) = new_raised();

        drop(lever);

        assert!(gate.lever_was_dropped());

        tokio_test::assert_ready_err!(tokio_test::task::spawn(gate.lowered()).poll());
    }

    /// Tests that `lowered` and `raised` will return without an `Err`
    /// - even if the `Lever` was dropped! -
    /// as long as the `Gate` was in the appropriate state
    /// when the `Lever` (the only way to change that state) dropped.
    #[test]
    fn ok_even_if_lever_dropped_for_matching_state() {
        let (raised_lever, mut raised_gate) = new_raised();
        let (lowered_lever, mut lowered_gate) = new_lowered();

        drop(raised_lever);
        drop(lowered_lever);

        tokio_test::assert_ready_ok!(tokio_test::task::spawn(lowered_gate.lowered()).poll());
        tokio_test::assert_ready_ok!(tokio_test::task::spawn(raised_gate.raised()).poll());
    }

    /// Tests that a `Lever` can check if its `Gate` dropped.
    #[test]
    fn lever_can_check_gate_was_dropped() {
        let (lever, gate) = new_raised();

        assert!(!lever.gate_was_dropped());

        drop(gate);

        assert!(lever.gate_was_dropped());
    }

    /// Tests that a `Gate` can check if its `Lever` dropped.
    #[test]
    fn gate_can_check_lever_was_dropped() {
        let (lever, gate) = new_raised();

        assert!(!gate.lever_was_dropped());

        drop(lever);

        assert!(gate.lever_was_dropped());
    }

    /// Tests that a `Lever` can retrieve the state of a `Gate`
    /// both before being dropped and after being dropped.
    #[test]
    fn lever_can_retrieve_dropped_gate_state() {
        let (lever, gate) = new_lowered();

        assert!(lever.is_lowered().unwrap());
        assert!(!lever.is_raised().unwrap());

        drop(gate);

        assert!(matches!(
            lever.is_lowered().unwrap_err(),
            BeforeGateDropped(Lowered)
        ));
        assert!(matches!(
            lever.is_raised().unwrap_err(),
            BeforeGateDropped(Lowered)
        ));
    }
}
