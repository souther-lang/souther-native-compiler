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
    /// Every key something is registered for on this thread, with what is registered. A list and
    /// not a map: a program injects a handful of behaviors, and a registration made around every
    /// call is a walk over a handful. Held as the addresses they are, and compared as addresses.
    static REGISTERED: RefCell<Vec<(*const Injection, *const Implementation)>> =
        const { RefCell::new(Vec::new()) };
}

/// What is registered for `key` on this thread, null where nothing is.
#[unsafe(no_mangle)]
pub extern "C" fn souther_injection_get(key: *const Injection) -> *const Implementation {
    REGISTERED.with_borrow(|registered| {
        registered
            .iter()
            .find(|(it, _)| std::ptr::eq(*it, key))
            .map_or(std::ptr::null(), |(_, implementation)| *implementation)
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
        let at = registered.iter().position(|(it, _)| std::ptr::eq(*it, key));
        match at {
            Some(at) if implementation.is_null() => registered.swap_remove(at).1,
            Some(at) => std::mem::replace(&mut registered[at].1, implementation),
            None if implementation.is_null() => std::ptr::null(),
            None => {
                registered.push((key, implementation));
                std::ptr::null()
            }
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn key(byte: &u8) -> *const Injection {
        (byte as *const u8).cast()
    }

    /// A distinct address to register, never called.
    fn implementation(at: &u8) -> *const Implementation {
        (at as *const u8).cast()
    }

    #[test]
    fn what_was_registered_is_given_back_when_it_is_replaced() {
        let (one, other, first, second) = (0u8, 0u8, 0u8, 0u8);
        assert!(souther_injection_get(key(&one)).is_null());
        assert!(souther_injection_exchange(key(&one), implementation(&first)).is_null());
        assert!(souther_injection_get(key(&other)).is_null());
        assert_eq!(
            souther_injection_exchange(key(&one), implementation(&second)),
            implementation(&first)
        );
        assert_eq!(souther_injection_get(key(&one)), implementation(&second));
        assert_eq!(
            souther_injection_exchange(key(&one), std::ptr::null()),
            implementation(&second)
        );
        assert!(souther_injection_get(key(&one)).is_null());
    }

    #[test]
    fn a_registration_is_the_threads_that_made_it() {
        let (one, first) = (0u8, 0u8);
        souther_injection_exchange(key(&one), implementation(&first));
        let elsewhere = std::thread::scope(|scope| {
            scope
                .spawn(|| souther_injection_get(key(&one)).is_null())
                .join()
                .unwrap()
        });
        assert!(elsewhere);
        souther_injection_exchange(key(&one), std::ptr::null());
    }
}
