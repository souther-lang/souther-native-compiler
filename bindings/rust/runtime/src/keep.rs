//! What a run keeps for the library to read until it ends: a function of the host's own made into
//! a function value, and the room that value is.

use crate::bound::{HOSTED_FUNCTION_SLOTS, Room};
use crate::native::Word;
use std::any::Any;
use std::cell::RefCell;
use std::collections::HashMap;
use std::ffi::c_void;

/// What a function value a host made of a function of its own is: `souther_hosted_function`,
/// laid out by the host and written by what makes the value, whose address is the value.
#[repr(C)]
pub struct HostedFunction {
    opaque: [u64; HOSTED_FUNCTION_SLOTS],
}

/// What makes a function value of a host's own function of one shape: `(room, implementation,
/// userdata) -> function`.
pub type FunctionImplementFn =
    unsafe extern "C" fn(*mut HostedFunction, *const c_void, *mut c_void) -> Word;

/// What one run keeps: the room of each function value made in it of a host's function, what that
/// function is handed first, and the value each such function was made into, so a function handed
/// over again is the value it was made into before.
#[derive(Default)]
pub(crate) struct Keeper {
    kept: RefCell<Kept>,
}

#[derive(Default)]
struct Kept {
    rooms: Vec<Box<dyn Any>>,
    made: HashMap<(usize, usize), Word>,
}

impl Keeper {
    /// The function value of `dispatch`, made by `implement` calling `implementation`, or the one it
    /// was made into already where `key` and `implementation` are what they were then.
    ///
    /// # Safety
    ///
    /// As [`crate::Run::host_function`].
    pub(crate) unsafe fn function<D: 'static>(
        &self,
        implement: FunctionImplementFn,
        implementation: *const c_void,
        key: usize,
        dispatch: D,
    ) -> Word {
        let at = (key, implementation as usize);
        if let Some(made) = self.kept.borrow().made.get(&at) {
            return *made;
        }
        let dispatch = Room::of(dispatch);
        let room = Room::of(HostedFunction {
            opaque: [0; HOSTED_FUNCTION_SLOTS],
        });
        // SAFETY: what the caller says; the room and the dispatch stay where they are until the
        // run that keeps them ends.
        let made = unsafe { implement(room.at(), implementation, dispatch.at().cast()) };
        let mut kept = self.kept.borrow_mut();
        kept.rooms.push(Box::new(dispatch));
        kept.rooms.push(Box::new(room));
        kept.made.insert(at, made);
        made
    }
}
