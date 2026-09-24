//! What a host registered for each behavior with no body, on each thread.
//!
//! Per thread, as the arena is: a host calls from the thread it registered on, and a runtime that
//! runs several requests at once on several threads (a ZTS PHP) registers on each. Nothing here
//! knows which behavior a key stands for or what an implementation takes. The object that answers
//! the behavior owns the key and calls the implementation; this only holds which one is where.

use std::cell::RefCell;

/// Which behavior an implementation is registered for: the address of a byte the answering object
/// holds for it, never read behind.
#[repr(C)]
pub struct Injection {
    _opaque: [u8; 0],
}

/// What a host registered: the address of a function it wrote, which the object calls and this
/// never does.
#[repr(C)]
pub struct Implementation {
    _opaque: [u8; 0],
}

thread_local! {
    /// Every key something is registered for on this thread. A list and not a map: a program
    /// injects a handful of behaviors, and a registration made around every call is a walk over a
    /// handful.
    static REGISTERED: RefCell<Vec<(usize, usize)>> = const { RefCell::new(Vec::new()) };
}

/// What is registered for `key` on this thread, null where nothing is.
#[unsafe(no_mangle)]
pub extern "C" fn souther_injection_get(key: *const Injection) -> *const Implementation {
    REGISTERED.with_borrow(|registered| {
        registered
            .iter()
            .find(|(it, _)| *it == key as usize)
            .map_or(std::ptr::null(), |(_, implementation)| {
                *implementation as *const Implementation
            })
    })
}

/// Registers `implementation` for `key` on this thread, null to register nothing, and answers what
/// was registered before, null where nothing was.
#[unsafe(no_mangle)]
pub extern "C" fn souther_injection_exchange(
    key: *const Injection,
    implementation: *const Implementation,
) -> *const Implementation {
    REGISTERED.with_borrow_mut(|registered| {
        let at = registered.iter().position(|(it, _)| *it == key as usize);
        let before = match at {
            Some(at) if implementation.is_null() => registered.swap_remove(at).1,
            Some(at) => std::mem::replace(&mut registered[at].1, implementation as usize),
            None if implementation.is_null() => 0,
            None => {
                registered.push((key as usize, implementation as usize));
                0
            }
        };
        before as *const Implementation
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn key(byte: &u8) -> *const Injection {
        (byte as *const u8).cast()
    }

    fn implementation(at: usize) -> *const Implementation {
        at as *const Implementation
    }

    #[test]
    fn what_was_registered_is_given_back_when_it_is_replaced() {
        let one = 0u8;
        let other = 0u8;
        assert!(souther_injection_get(key(&one)).is_null());
        assert!(souther_injection_exchange(key(&one), implementation(8)).is_null());
        assert!(souther_injection_get(key(&other)).is_null());
        assert_eq!(
            souther_injection_exchange(key(&one), implementation(16)),
            implementation(8)
        );
        assert_eq!(souther_injection_get(key(&one)), implementation(16));
        assert_eq!(
            souther_injection_exchange(key(&one), std::ptr::null()),
            implementation(16)
        );
        assert!(souther_injection_get(key(&one)).is_null());
    }

    #[test]
    fn a_registration_is_the_threads_that_made_it() {
        let one = 0u8;
        let at = key(&one) as usize;
        souther_injection_exchange(key(&one), implementation(8));
        let elsewhere =
            std::thread::spawn(move || souther_injection_get(at as *const Injection).is_null())
                .join()
                .unwrap();
        assert!(elsewhere);
        souther_injection_exchange(key(&one), std::ptr::null());
    }
}
