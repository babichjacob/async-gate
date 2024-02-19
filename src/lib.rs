use thiserror::Error;
use tokio::sync::watch;

pub const RAISED: bool = true;
pub const LOWERED: bool = false;

/// The gate was dropped, but we still know what value it had before dropping
#[derive(Debug, Error)]
#[error("gate was {0} (where true is RAISED and false is LOWERED) before dropping")]
pub struct BeforeGateDropped(bool);

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
        if self.sender.is_closed() {
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
        if self.sender.is_closed() {
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

        if self.sender.is_closed() {
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

        if self.sender.is_closed() {
            Err(BeforeGateDropped(state))
        } else {
            Ok(state)
        }
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
