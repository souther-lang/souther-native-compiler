//! What a run keeps for the library to read until it ends: a function of the host's own made into
//! a function value, and the room that value is.

use crate::bound::{Hosted, Room};
use crate::native::Word;
use crate::run::Value;
use std::any::Any;
use std::cell::RefCell;
use std::collections::HashMap;
use std::ffi::c_void;
use std::ptr;

/// What a function value a host made of a function of its own is: `souther_hosted_function`,
/// laid out by the host and written by what makes the value, whose address is the value.
#[repr(C)]
pub struct HostedFunction {
    invoke: *const c_void,
    hosted: Hosted,
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
            invoke: ptr::null(),
            hosted: Hosted::empty(),
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

/// A value the library made that the host holds by its address alone: a function value it
/// answered, which a host calls through the library of `L`.
pub struct Held<'run, L> {
    value: Value<'run>,
    library: &'run L,
}

impl<L> Clone for Held<'_, L> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<L> Copy for Held<'_, L> {}

impl<'run, L> Held<'run, L> {
    /// The value at `at`, which `library` answered.
    ///
    /// # Safety
    ///
    /// `at` is an address `library` answered that stays good for as long as `'run`.
    pub unsafe fn new(library: &'run L, at: Word) -> Self {
        let at = ptr::NonNull::new(at.cast_mut()).expect("the library answers a value's address");
        Held {
            // SAFETY: what the caller says.
            value: unsafe { Value::from_address(at) },
            library,
        }
    }

    /// Where the value stands, to hand to the library.
    pub fn word(&self) -> Word {
        self.value.address().as_ptr().cast_const()
    }

    /// The library that made it.
    pub fn library(&self) -> &'run L {
        self.library
    }
}
