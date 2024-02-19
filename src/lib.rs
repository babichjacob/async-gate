use thiserror::Error;
use tokio::sync::watch;

pub const RAISED: bool = true;
pub const LOWERED: bool = false;

/// The gate was dropped, but we still know what value it had before dropping
#[derive(Debug, Error)]
#[error("gate was {0} (where true is RAISED and false is LOWERED) before dropping")]
pub struct BeforeGateDropped(pub bool);

/// The gate was dropped, so raising or lowering it achieves nothing
#[derive(Debug, Error)]
#[error("gate was dropped")]
pub struct GateDropped;

/// The lever was dropped, so waiting any longer for the gate to be raised or lowered can't be meaningful
#[derive(Debug, Error)]
#[error("lever was dropped")]
pub struct LeverDropped;

/// A lever that can [`raise`] and [`lower`] the gate it's associated with
pub struct Lever {
    sender: watch::Sender<bool>,
}

impl Lever {
    /// Raise the gate.
    /// This wakes all tasks waiting on [`Gate::raised`]
    /// (and later calls to it will resolve immediately until the gate is [`lower`]ed).
    pub fn raise(&self) -> Result<(), GateDropped> {
        if self.gate_was_dropped() {
            Err(GateDropped)
        } else {
            self.sender.send_if_modified(|state| {
                if *state == LOWERED {
                    *state = RAISED;
                    true
                } else {
                    false
                }
            });

            Ok(())
        }
    }

    /// Lower the gate.
    /// This wakes all tasks waiting on [`Gate::lowered`]
    /// (and later calls to it will resolve immediately until the gate is [`raise`]d).
    pub fn lower(&self) -> Result<(), GateDropped> {
        if self.gate_was_dropped() {
            Err(GateDropped)
        } else {
            self.sender.send_if_modified(|state| {
                if *state == RAISED {
                    *state = LOWERED;
                    true
                } else {
                    false
                }
            });

            Ok(())
        }
    }

    /// Returns `Ok(true)` if the gate is raised and `Ok(false)` if it's lowered,
    /// or `Err(BeforeGateDropped(true))` if the gate was dropped and was raised before dropping
    /// and `Err(BeforeGateDropped(false))` if the gate was dropped and was lowered before dropping.
    pub fn is_raised(&self) -> Result<bool, BeforeGateDropped> {
        let state = *self.sender.borrow() == RAISED;

        if self.gate_was_dropped() {
            Err(BeforeGateDropped(state))
        } else {
            Ok(state)
        }
    }

    /// Returns `Ok(true)` if the gate is lowered and `Ok(false)` if it's raised,
    /// or `Err(BeforeGateDropped(true))` if the gate was dropped and was lowered before dropping
    /// and `Err(BeforeGateDropped(false))` if the gate was dropped and was raised before dropping.
    pub fn is_lowered(&self) -> Result<bool, BeforeGateDropped> {
        let state = *self.sender.borrow() == LOWERED;

        if self.gate_was_dropped() {
            Err(BeforeGateDropped(state))
        } else {
            Ok(state)
        }
    }

    /// Returns `true` if the gate associated with this lever has been dropped
    /// and `false` if it hasn't.
    pub fn gate_was_dropped(&self) -> bool {
        self.sender.is_closed()
    }
}

/// A gate that can be checked if [`is_raised`] or [`is_lowered`] immediately,
/// or can be waited on to be [`raised`] or [`lowered`].
#[derive(Clone)]
pub struct Gate {
    receiver: watch::Receiver<bool>,
}

impl Gate {
    /// Returns true if the gate (even if the lever has been dropped) is raised and false if it's lowered.
    pub fn is_raised(&self) -> bool {
        *self.receiver.borrow() == RAISED
    }

    /// Returns true if the gate (even if the lever has been dropped) is lowered and false if it's raised.
    pub fn is_lowered(&self) -> bool {
        *self.receiver.borrow() == LOWERED
    }

    /// Wait until the gate is raised
    /// (by a call to [`Lever::raise`])
    pub async fn raised(&mut self) -> Result<(), LeverDropped> {
        match self.receiver.wait_for(|state| *state == RAISED).await {
            Ok(_) => Ok(()),
            Err(_) => Err(LeverDropped),
        }
    }

    /// Wait until the gate is lowered
    /// (by a call to [`Lever::lower`])
    pub async fn lowered(&mut self) -> Result<(), LeverDropped> {
        match self.receiver.wait_for(|state| *state == LOWERED).await {
            Ok(_) => Ok(()),
            Err(_) => Err(LeverDropped),
        }
    }

    /// Returns `true` if the lever associated with this gate has been dropped
    /// and `false` if it hasn't.
    pub fn lever_was_dropped(&self) -> bool {
        self.receiver.has_changed().is_err()
    }
}

/// Create a [`Gate`] that is initially raised.
/// The [`Lever`] that it is returned with can raise and lower the gate.
pub fn new_raised() -> (Lever, Gate) {
    let (sender, receiver) = watch::channel(RAISED);

    let lever = Lever { sender };
    let gate = Gate { receiver };

    (lever, gate)
}

/// Create a [`Gate`] that is initially lowered.
/// The [`Lever`] that it is returned with can raise and lower the gate.
pub fn new_lowered() -> (Lever, Gate) {
    let (sender, receiver) = watch::channel(LOWERED);

    let lever = Lever { sender };
    let gate = Gate { receiver };

    (lever, gate)
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
    /// when the `Gate` is already `RAISED` or `LOWERED` respectively.
    #[test]
    fn resolves_instantly() {
        let (_raised_lever, mut raised_gate) = new_raised();
        let (_lowered_lever, mut lowered_gate) = new_lowered();

        tokio_test::assert_ready!(tokio_test::task::spawn(raised_gate.raised()).poll()).unwrap();
        tokio_test::assert_ready!(tokio_test::task::spawn(lowered_gate.lowered()).poll()).unwrap();
    }

    /// Tests that `lowered` and `raised` do not resolve instantly
    /// when the `Gate` is currently `RAISED` or `LOWERED` respectively.
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

    /// Tests that calling `raised` on a `Gate` that was `LOWERED` when its `Lever` dropped results in an `Err`.
    #[test]
    fn lowered_gate_gives_err_on_raised_when_lever_dropped() {
        let (lever, mut gate) = new_lowered();

        drop(lever);

        assert!(gate.lever_was_dropped());

        tokio_test::assert_ready_err!(tokio_test::task::spawn(gate.raised()).poll());
    }

    /// Tests that calling `lowered` on a `Gate` that was `RAISED` when its `Lever` dropped results in an `Err`.
    #[test]
    fn raised_gate_gives_err_on_lowered_when_lever_dropped() {
        let (lever, mut gate) = new_raised();

        drop(lever);

        assert!(gate.lever_was_dropped());

        tokio_test::assert_ready_err!(tokio_test::task::spawn(gate.lowered()).poll());
    }

    /// Tests that `lowered` and `raised` will return without an `Err`
    /// - even if the `Lever` was dropped! -
    /// as long as the `Gate` is in the appropriate state
    /// before the `Lever` (the only way to change that state) dropped.
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

        assert!(lever.is_lowered().unwrap_err().0);
        assert!(!lever.is_raised().unwrap_err().0);
    }
}
