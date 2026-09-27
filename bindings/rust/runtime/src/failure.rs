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
            .map(|&(name, status)| Failure::Abort(Abort { status, name: Some(name) }))
            .ok_or(UnnamedStatus("REQUIRED_FORM_HAS_NO_PLACE"))
    }

    /// What a host implementation that answered answers the library.
    pub fn answered_status(&self) -> Status {
        self.answered
    }

    /// What a call that answered `status` comes to, where a host implementation it called back
    /// left `caught`: a panic raised again, and a failure answered, whatever the status.
    pub(crate) fn answered(&self, status: Status, caught: Option<Caught>) -> Result<(), Failure> {
        if let Some(caught) = caught {
            return Err(Failure::Host(caught.raised()));
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
