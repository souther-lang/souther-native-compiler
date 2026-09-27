//! What a behavior is bound to: the capabilities of what it requires, laid out as the library
//! reads them.

use crate::failure::Failure;
use crate::run::Runtime;
use std::ffi::c_void;
use std::marker::PhantomData;
use std::ptr::{self, NonNull};

/// One capability, as a host lays out room for one and the library writes it: `souther_capability`.
#[repr(C)]
pub struct Capability {
    invoke: *const c_void,
    environment: *const c_void,
}

/// What a capability of a host's own implementation reads it out of: `souther_hosted`.
#[repr(C)]
pub struct Hosted {
    implementation: *const c_void,
    userdata: *mut c_void,
}

impl Hosted {
    pub(crate) fn empty() -> Self {
        Hosted {
            implementation: ptr::null(),
            userdata: ptr::null_mut(),
        }
    }
}

impl Capability {
    fn room() -> Room<Self> {
        Room::of(Capability {
            invoke: ptr::null(),
            environment: ptr::null(),
        })
    }
}

/// Room this crate laid out and hands the library the address of, which stays where it is until
/// this is dropped.
///
/// Held as the address and not as a `Box`: a `Box` moved while the library holds the address would
/// assert that nothing else reaches what it owns, which the library does.
pub(crate) struct Room<T: ?Sized>(NonNull<T>);

impl<T> Room<T> {
    pub(crate) fn of(it: T) -> Self {
        Room(NonNull::from(Box::leak(Box::new(it))))
    }
}

impl<T> Room<[T]> {
    fn of_slice(it: Box<[T]>) -> Self {
        Room(NonNull::from(Box::leak(it)))
    }
}

impl<T: ?Sized> Room<T> {
    pub(crate) fn at(&self) -> *mut T {
        self.0.as_ptr()
    }
}

impl<T: ?Sized> Drop for Room<T> {
    fn drop(&mut self) {
        // SAFETY: the room was a box leaked in `of`, and nothing but this frees it.
        drop(unsafe { Box::from_raw(self.0.as_ptr()) });
    }
}

/// What makes the capability of a behavior constructed from its requirements: `(into,
/// requirements)`.
pub type BindFn = unsafe extern "C" fn(*mut Capability, *const *const Capability);

/// What makes the capability of a host's own implementation of a behavior: `(into, hosted,
/// implementation, userdata)`.
pub type ImplementFn =
    unsafe extern "C" fn(*mut Capability, *mut Hosted, *const c_void, *mut c_void);

/// Which runtime a capability was made by, which is the only one it may be handed to.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Made(usize);

impl Made {
    fn by(runtime: &Runtime) -> Self {
        Made(runtime.identity())
    }
}

/// What stands for a behavior another requires: a behavior bound in turn, or a host's own
/// implementation of one.
pub trait Requirement {
    /// Its capability, as the library reads it for as long as `self` lives.
    fn capability(&self) -> NonNull<Capability>;

    /// The runtime that made it and everything it stands on, or none where they are more than
    /// one: a behavior bound to what another runtime made, at any depth.
    fn made(&self) -> Option<Made>;
}

/// A behavior bound to what stands for each behavior it requires, in the order it requires them,
/// and its own capability, where something may require it.
///
/// Borrows what it is bound to for `'a`: the library reads their capabilities wherever the
/// behavior runs, and never copies them.
pub struct Bound<'a> {
    made: Option<Made>,
    requirements: Room<[*const Capability]>,
    capability: Option<Room<Capability>>,
    _requires: PhantomData<&'a ()>,
}

impl<'a> Bound<'a> {
    /// The behavior bound to `requires` by `runtime`'s library, whose capability `bind` makes where
    /// something may require it.
    ///
    /// # Safety
    ///
    /// `bind` is the library's function making the capability of this behavior, and `requires`
    /// stands for what it requires, in order.
    ///
    /// What another runtime made may be among `requires`: binding only lays out addresses and
    /// calls nothing, and a call is what refuses it ([`Bound::requirements`]), as a call is what
    /// refuses a value another runtime made.
    pub unsafe fn new(
        runtime: &Runtime,
        bind: Option<BindFn>,
        requires: &[&'a dyn Requirement],
    ) -> Self {
        let made = Made::by(runtime);
        let made = requires
            .iter()
            .all(|required| required.made() == Some(made))
            .then_some(made);
        let requirements = Room::of_slice(
            requires
                .iter()
                .map(|it| it.capability().as_ptr().cast_const())
                .collect(),
        );
        let from = Self::first(&requirements);
        let capability = bind.map(|bind| {
            let room = Capability::room();
            // SAFETY: what the caller says; the room and the requirements stay where they are for
            // as long as `self`.
            unsafe { bind(room.at(), from) };
            room
        });
        Bound {
            made,
            requirements,
            capability,
            _requires: PhantomData,
        }
    }

    /// What a call of the behavior into `runtime`'s library is handed first: the address of the
    /// capabilities of what it requires, or null where it requires nothing.
    ///
    /// # Errors
    ///
    /// [`Failure::Foreign`] where another runtime bound it, or bound anything it stands on: the
    /// call would run another library's code over this one's arena.
    pub fn requirements(&self, runtime: &Runtime) -> Result<*const *const Capability, Failure> {
        if self.made == Some(Made::by(runtime)) {
            Ok(Self::first(&self.requirements))
        } else {
            Err(Failure::Foreign)
        }
    }

    fn first(requirements: &Room<[*const Capability]>) -> *const *const Capability {
        if requirements.0.is_empty() {
            ptr::null()
        } else {
            requirements.at().cast::<*const Capability>().cast_const()
        }
    }
}

impl Requirement for Bound<'_> {
    fn capability(&self) -> NonNull<Capability> {
        let capability = self
            .capability
            .as_ref()
            .expect("only a behavior something may require is required");
        capability.0
    }

    fn made(&self) -> Option<Made> {
        self.made
    }
}

/// A host's own implementation of a behavior, made into a capability the library calls it through.
///
/// `D` is what the implementation's function is handed first, which says what implementation it
/// calls. It, the room the library reads it out of and the capability are kept here, where they do
/// not move, for as long as this is.
pub struct Implemented<D> {
    made: Made,
    capability: Room<Capability>,
    _hosted: Room<Hosted>,
    _dispatch: Room<D>,
}

impl<D> Implemented<D> {
    /// The capability of an implementation that `implementation` calls, handed `dispatch` first,
    /// made by `implement`.
    ///
    /// # Safety
    ///
    /// `implement` is `runtime`'s library's function making the capability of the behavior, and
    /// `implementation` is a function of the type that behavior's implementation is, reading what
    /// it is handed first as a `D`.
    pub unsafe fn new(
        runtime: &Runtime,
        implement: ImplementFn,
        implementation: *const c_void,
        dispatch: D,
    ) -> Self {
        let dispatch = Room::of(dispatch);
        let hosted = Room::of(Hosted::empty());
        let capability = Capability::room();
        // SAFETY: what the caller says; all three stay where they are for as long as `self`.
        unsafe {
            implement(
                capability.at(),
                hosted.at(),
                implementation,
                dispatch.at().cast(),
            );
        }
        Implemented {
            made: Made::by(runtime),
            capability,
            _hosted: hosted,
            _dispatch: dispatch,
        }
    }
}

impl<D> Requirement for Implemented<D> {
    fn capability(&self) -> NonNull<Capability> {
        self.capability.0
    }

    fn made(&self) -> Option<Made> {
        Some(self.made)
    }
}
