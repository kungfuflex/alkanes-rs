use alkanes_support::id::AlkaneId;
use alkanes_support::trace::{TraceContext, TraceEvent};
use metashrew_core::index_pointer::AtomicPointer;

pub trait Extcall {
    fn isdelegate() -> bool;
    fn isstatic() -> bool;
    fn event(context: TraceContext) -> TraceEvent;
    fn handle_atomic(atomic: &mut AtomicPointer) {
        if Self::isstatic() {
            atomic.rollback();
        } else {
            atomic.commit();
        }
    }
    fn change_context(
        target: AlkaneId,
        caller: AlkaneId,
        myself: AlkaneId,
    ) -> (AlkaneId, AlkaneId) {
        if Self::isdelegate() {
            (caller, myself)
        } else {
            (myself, target)
        }
    }
}

pub struct Call(());

impl Extcall for Call {
    fn isdelegate() -> bool {
        false
    }
    fn isstatic() -> bool {
        false
    }
    fn event(context: TraceContext) -> TraceEvent {
        TraceEvent::EnterCall(context)
    }
}

pub struct Delegatecall(());

impl Extcall for Delegatecall {
    fn isdelegate() -> bool {
        true
    }
    fn isstatic() -> bool {
        false
    }
    fn event(context: TraceContext) -> TraceEvent {
        TraceEvent::EnterDelegatecall(context)
    }
}

pub struct Staticcall(());

impl Extcall for Staticcall {
    fn isdelegate() -> bool {
        false
    }
    fn isstatic() -> bool {
        true
    }
    fn event(context: TraceContext) -> TraceEvent {
        TraceEvent::EnterStaticcall(context)
    }
}
