//! What a call into a library comes to where it answers no value.

use std::any::Any;
use std::fmt;

/// What a function of the library answers: whether it answered, and why not.
pub type Status = u32;

/// What a host implementation answers in place of a value, and what a call it was reached from
/// then answers ([`Failure::Host`]).
pub type HostError = Box<dyn std::error::Error + Send + Sync + 'static>;

/// What each status a library answers is, by the names its manifest gives them.
#[derive(Debug, Clone, Copy)]
pub struct Statuses {
    answered: Status,
    unbound: Status,
    protocol_violation: Status,
    host_exception: Status,
    named: &'static [(&'static str, Status)],
}

impl Statuses {
    /// The statuses `named` names, as a manifest says them.
    ///
    /// # Errors
    ///
    /// [`UnnamedStatus`] where one a host has to tell apart is not among them.
    pub fn new(named: &'static [(&'static str, Status)]) -> Result<Self, UnnamedStatus> {
        let of = |name: &'static str| {
            named
                .iter()
                .find(|(it, _)| *it == name)
                .map(|(_, status)| *status)
                .ok_or(UnnamedStatus(name))
        };
        Ok(Statuses {
            answered: of("ANSWERED")?,
            unbound: of("INJECTION_UNBOUND")?,
            protocol_violation: of("INJECTION_PROTOCOL_VIOLATION")?,
            host_exception: of("HOST_EXCEPTION")?,
            named,
        })
    }

    /// What a host implementation that failed answers the library.
    pub fn host_exception(&self) -> Status {
        self.host_exception
    }

    /// [`Failure::Abort`] for `REQUIRED_FORM_HAS_NO_PLACE`, the status a library answers where
    /// text a host hands it has no place as a `String` (spec §what-a-string-holds) — what
    /// `souther_string_of_utf8` and `Words::string` answer `false`/`Err` for, in place of a raw
    /// status number that call site would otherwise have to name by hand.
    ///
    /// # Errors
    ///
    /// [`UnnamedStatus`] where the library's manifest does not name it. Every library a host asks
    /// to build a `String` from bytes has to; where it does not, that is itself a manifest that
    /// does not match what this binding calls, the same as any other [`UnnamedStatus`].
    pub fn no_place(&self) -> Result<Failure, UnnamedStatus> {
        self.named
            .iter()
            .find(|(name, _)| *name == "REQUIRED_FORM_HAS_NO_PLACE")
            .map(|&(name, status)| {
                Failure::Abort(Abort {
                    status,
                    name: Some(name),
                })
            })
            .ok_or(UnnamedStatus("REQUIRED_FORM_HAS_NO_PLACE"))
    }

    /// What a host implementation that answered answers the library.
    pub fn answered_status(&self) -> Status {
        self.answered
    }

    /// What a call that answered `status` comes to, where a host implementation it called back
    /// left `caught`: a panic raised again, and a failure answered, whatever the status.
    ///
    /// A `caught` failure a generated `implemented` closure raised is boxed as a
    /// [`CallbackFailure`], never a bare [`HostError`] or [`Failure`], because which of the two
    /// happened cannot be told apart by the payload's type alone — a host implementation may
    /// legitimately answer a `Failure` as its own error. Where it downcasts to one,
    /// [`CallbackFailure::Host`] answers [`Failure::Host`] and [`CallbackFailure::Crossing`]
    /// unwraps straight to the `Failure` it holds — most often `Failure::Abort` where crossing a
    /// `String` found no place (souther-native-compiler#109), or `Failure::Foreign` where a handle
    /// crossed from another runtime — rather than doubly wrapping it as a `Failure::Host` of a
    /// `Failure`: the crossing failed, not the implementation, and a caller matching on
    /// `Failure::Abort` needs the same shape a direct call into the library would answer for the
    /// same reason. A `caught` that does not downcast to `CallbackFailure` at all — anything not
    /// produced through this generated shape — answers `Failure::Host` unchanged, as before.
    pub(crate) fn answered(&self, status: Status, caught: Option<Caught>) -> Result<(), Failure> {
        if let Some(caught) = caught {
            return Err(match caught.raised().downcast::<CallbackFailure>() {
                Ok(callback) => match *callback {
                    CallbackFailure::Host(error) => Failure::Host(error),
                    CallbackFailure::Crossing(failure) => failure,
                },
                Err(raised) => Failure::Host(raised),
            });
        }
        match status {
            it if it == self.answered => Ok(()),
            it if it == self.host_exception => Err(Failure::ProtocolViolation),
            it if it == self.protocol_violation => Err(Failure::ProtocolViolation),
            it if it == self.unbound => Err(Failure::Unbound),
            it => Err(Failure::Abort(Abort {
                status: it,
                name: self
                    .named
                    .iter()
                    .find(|(_, status)| *status == it)
                    .map(|(name, _)| *name),
            })),
        }
    }
}

/// A status a host has to tell apart that a manifest does not name.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct UnnamedStatus(pub &'static str);

impl fmt::Display for UnnamedStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "the library numbers no status {}", self.0)
    }
}

impl std::error::Error for UnnamedStatus {}

/// Why a call into a library answered no value.
#[derive(Debug)]
pub enum Failure {
    /// The computation ended without one, for the reason the library numbers.
    Abort(Abort),
    /// A host implementation it called back answered a failure of its own, which this is.
    Host(HostError),
    /// A behavior was reached through a requirement it was handed nothing for.
    Unbound,
    /// A host implementation answered what it may not, or said it failed and nothing was kept.
    ProtocolViolation,
    /// A value, a function value or what stands for a required behavior that another library's
    /// runtime made was handed to this one's, and the call was not made.
    Foreign,
}

impl fmt::Display for Failure {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Failure::Abort(abort) => write!(f, "{abort}"),
            Failure::Host(failure) => write!(f, "a host implementation failed: {failure}"),
            Failure::Unbound => {
                f.write_str("a behavior was reached through a requirement nothing was handed for")
            }
            Failure::ProtocolViolation => f.write_str(
                "a host implementation answered something other than a value or a failure",
            ),
            Failure::Foreign => f.write_str(
                "what another library made was handed to a call into this one, which was not made",
            ),
        }
    }
}

impl std::error::Error for Failure {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Failure::Host(failure) => Some(failure.as_ref()),
            _ => None,
        }
    }
}

/// Why a generated callback — `implemented`'s closure, over a host implementation's own answer or
/// a function value's — answered no value: the host implementation's own error, or a [`Failure`]
/// a generated argument or answer conversion raised crossing back into the library.
///
/// A callback is generated as `Result<(), HostError>`, and [`Failure`] is itself a public
/// [`std::error::Error`], so a host implementation may legitimately answer a `Failure` as its own
/// error: which of the two happened is not a question a payload's type alone can answer. Generated
/// code boxes each explicitly as this instead — the host implementation's own error as
/// [`CallbackFailure::Host`], the one a crossing back into the library raised (a generated
/// argument or answer conversion's own `?`, `library.words.string` and a declared type's
/// `__word` among them) as [`CallbackFailure::Crossing`] — so [`Statuses::answered`] can tell them
/// apart by which variant it downcasts to rather than by inspecting what it holds
/// (souther-native-compiler#109).
#[derive(Debug)]
pub enum CallbackFailure {
    /// The host implementation's own error.
    Host(HostError),
    /// A [`Failure`] a crossing back into the library raised, and not the implementation's own.
    Crossing(Failure),
}

impl fmt::Display for CallbackFailure {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            CallbackFailure::Host(error) => write!(f, "a host implementation failed: {error}"),
            CallbackFailure::Crossing(failure) => {
                write!(f, "a crossing back into the library failed: {failure}")
            }
        }
    }
}

impl std::error::Error for CallbackFailure {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            CallbackFailure::Host(error) => Some(error.as_ref()),
            CallbackFailure::Crossing(failure) => Some(failure),
        }
    }
}

/// Runs `f`, the crossing of a host implementation's answer back into the library, boxing a
/// `Failure` it raises as [`CallbackFailure::Crossing`] rather than propagating it with `?`
/// straight into the surrounding callback's `Result<(), HostError>`: that would leave `answered`
/// unable to tell it apart from the implementation's own error the same way it can't tell a
/// `Failure` a host chose as its own error apart from one a crossing raised
/// (souther-native-compiler#109).
///
/// Generated code calls this rather than declaring and calling its own closure inline, which
/// `clippy::redundant_closure_call` refuses: `f` is genuinely handed to something else here, a
/// helper of the protocol between generated code and this crate, and not a closure declared only
/// to be called where it stands.
///
/// # Errors
///
/// Whatever `f` answers, boxed as [`CallbackFailure::Crossing`].
pub fn crossing(f: impl FnOnce() -> Result<(), Failure>) -> Result<(), HostError> {
    f().map_err(|failure| Box::new(CallbackFailure::Crossing(failure)) as HostError)
}

/// A computation that ended without a value: the status, and the name the library gives it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Abort {
    status: Status,
    name: Option<&'static str>,
}

impl Abort {
    /// The status the library answered.
    pub fn status(&self) -> Status {
        self.status
    }

    /// What the library calls it (`INVARIANT_NOT_HELD`, `DIVISION_BY_ZERO`, ...), where it names
    /// it.
    pub fn name(&self) -> Option<&'static str> {
        self.name
    }
}

impl fmt::Display for Abort {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.name {
            Some(name) => write!(f, "the computation ended with {name}"),
            None => write!(f, "the computation ended with status {}", self.status),
        }
    }
}

/// What a host implementation left for the call it was reached from.
pub(crate) enum Caught {
    Panicked(Box<dyn Any + Send>),
    Failed(HostError),
}

#[cfg(test)]
mod tests {
    use super::*;

    const BASE: &[(&str, Status)] = &[
        ("ANSWERED", 0),
        ("INJECTION_UNBOUND", 1),
        ("INJECTION_PROTOCOL_VIOLATION", 2),
        ("HOST_EXCEPTION", 3),
    ];

    const WITH_NO_PLACE: &[(&str, Status)] = &[
        ("ANSWERED", 0),
        ("INJECTION_UNBOUND", 1),
        ("INJECTION_PROTOCOL_VIOLATION", 2),
        ("HOST_EXCEPTION", 3),
        ("REQUIRED_FORM_HAS_NO_PLACE", 7),
    ];

    /// `crossing` boxes what `f` raises as `CallbackFailure::Crossing`, downcastable back out
    /// exactly as `answered` downcasts a caught one — the two ends of the same protocol, tested
    /// without a loaded library.
    #[test]
    fn crossing_boxes_a_raised_failure_as_callback_failure_crossing() {
        let statuses = Statuses::new(WITH_NO_PLACE).unwrap();
        let no_place = statuses.no_place().unwrap();
        let boxed = crossing(|| Err(no_place)).unwrap_err();
        let Ok(callback) = boxed.downcast::<CallbackFailure>() else {
            panic!("crossing boxes what f raises as CallbackFailure");
        };
        let CallbackFailure::Crossing(Failure::Abort(abort)) = *callback else {
            panic!("expected CallbackFailure::Crossing(Failure::Abort(..))");
        };
        assert_eq!(abort.name(), Some("REQUIRED_FORM_HAS_NO_PLACE"));
    }

    /// `crossing` answers `Ok(())` unchanged where `f` does.
    #[test]
    fn crossing_of_a_success_is_ok() {
        assert!(crossing(|| Ok(())).is_ok());
    }

    /// `no_place` reads the manifest's own number for `REQUIRED_FORM_HAS_NO_PLACE` and names it,
    /// rather than a number this binding picks: two libraries that number their statuses
    /// differently still answer the same `Abort` name for the same reason
    /// (souther-native-compiler#109).
    #[test]
    fn no_place_is_the_manifests_own_status_named() {
        let statuses = Statuses::new(WITH_NO_PLACE).unwrap();
        let Failure::Abort(abort) = statuses.no_place().unwrap() else {
            panic!("no_place answers an Abort");
        };
        assert_eq!(abort.status(), 7);
        assert_eq!(abort.name(), Some("REQUIRED_FORM_HAS_NO_PLACE"));
    }

    /// A `Failure` a generated argument or answer conversion raised with `?` inside a host
    /// implementation's callback — `Words::string` answering `Err(Failure::Abort(..))` where an
    /// injected behavior's own answer has no place as a `String`, most concretely — crosses the
    /// callback boundary boxed as `CallbackFailure::Crossing`. `answered` unwraps it back to the
    /// original `Failure` rather than wrapping it again as `Failure::Host`: the crossing failed,
    /// not the implementation, and a caller matching on `Failure::Abort` (as `Construction::of`
    /// already does for `INVARIANT_NOT_HELD`) has to see the same shape here as it would calling
    /// directly (souther-native-compiler#109).
    #[test]
    fn a_crossing_failure_caught_across_a_callback_is_unwrapped_and_not_doubly_wrapped() {
        let statuses = Statuses::new(WITH_NO_PLACE).unwrap();
        let no_place = statuses.no_place().unwrap();
        let caught = Caught::Failed(Box::new(CallbackFailure::Crossing(no_place)));
        let answered = statuses.answered(0, Some(caught));
        let Err(Failure::Abort(abort)) = answered else {
            panic!("expected Failure::Abort straight through, got {answered:?}");
        };
        assert_eq!(abort.name(), Some("REQUIRED_FORM_HAS_NO_PLACE"));
    }

    /// The case a type-based downcast to `Failure` cannot tell apart from the one above: a host
    /// implementation legitimately answering a `Failure` as its own error — `Failure` is a public
    /// `Error`, so nothing stops a host from reusing it. Boxed as `CallbackFailure::Host` (what
    /// generated code does for the implementation's own `Result`, never `CallbackFailure::Crossing`,
    /// regardless of what the error's own type happens to be), this answers `Failure::Host`
    /// wrapping the original `Failure` — never unwrapped as if the crossing itself had failed.
    /// Provenance is which variant of `CallbackFailure` generated code chose at the point the error
    /// was boxed, not something inferred from the payload afterward (souther-native-compiler#109).
    #[test]
    fn a_hosts_own_failure_shaped_error_still_answers_failure_host() {
        let statuses = Statuses::new(WITH_NO_PLACE).unwrap();
        let no_place = statuses.no_place().unwrap();
        let caught = Caught::Failed(Box::new(CallbackFailure::Host(Box::new(no_place))));
        let answered = statuses.answered(0, Some(caught));
        let Err(Failure::Host(host)) = answered else {
            panic!("expected Failure::Host wrapping the host's own Failure, got {answered:?}");
        };
        assert!(
            host.downcast::<Failure>().is_ok(),
            "the host's own Failure survives inside it"
        );
    }

    /// A host implementation's own error that was never boxed as a `CallbackFailure` at all —
    /// reaching `answered` this way only through a `Caught` no generated code produced — still
    /// answers `Failure::Host`, the same fallback `answered` already gave every `caught` before
    /// `CallbackFailure` existed.
    #[test]
    fn an_error_that_is_not_a_callback_failure_still_answers_failure_host() {
        let statuses = Statuses::new(BASE).unwrap();
        #[derive(Debug)]
        struct Mine;
        impl std::fmt::Display for Mine {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                f.write_str("mine")
            }
        }
        impl std::error::Error for Mine {}
        let caught = Caught::Failed(Box::new(Mine));
        let answered = statuses.answered(0, Some(caught));
        assert!(matches!(answered, Err(Failure::Host(_))), "{answered:?}");
    }

    /// A manifest that never names `REQUIRED_FORM_HAS_NO_PLACE` — a library built before
    /// souther-native-compiler#109, or one that never builds a `String` from a host's bytes —
    /// answers `UnnamedStatus` rather than a wrong `Abort`: `Words::string` (native.rs) turns this
    /// into `Failure::ProtocolViolation` rather than propagating a status-table lookup error where
    /// its own callers expect a `Failure`.
    #[test]
    fn no_place_is_unnamed_where_the_manifest_does_not_name_it() {
        let statuses = Statuses::new(BASE).unwrap();
        let Err(unnamed) = statuses.no_place() else {
            panic!("no_place answers UnnamedStatus where the manifest never names it");
        };
        assert_eq!(unnamed, UnnamedStatus("REQUIRED_FORM_HAS_NO_PLACE"));
    }
}
