package souther

import (
	"errors"
	"fmt"
)

// Status is what a function of the library answers: whether it answered, and why not.
type Status = uint32

// statusTable is what each status a library answers is, by the names its manifest gives them.
type statusTable struct {
	answered          Status
	unbound           Status
	protocolViolation Status
	hostException     Status
	named             map[string]Status
	names             map[Status]string
}

// UnnamedOutcome is what a reading comes to that a host has to tell apart and a manifest does not
// name.
type UnnamedOutcome struct{ Name string }

func (e *UnnamedOutcome) Error() string {
	return fmt.Sprintf("the library numbers no outcome %s of a reading", e.Name)
}

// UnnamedStatus is a status a host has to tell apart that a manifest does not name.
type UnnamedStatus struct{ Name string }

func (e *UnnamedStatus) Error() string {
	return fmt.Sprintf("the library numbers no status %s", e.Name)
}

// newStatuses reads the statuses a manifest names.
//
// It returns an [*UnnamedStatus] where one a host has to tell apart is not among them.
func newStatuses(named map[string]Status) (statusTable, error) {
	of := func(name string) (Status, error) {
		status, ok := named[name]
		if !ok {
			return 0, &UnnamedStatus{name}
		}
		return status, nil
	}
	s := statusTable{named: make(map[string]Status, len(named)), names: make(map[Status]string, len(named))}
	for name, status := range named {
		s.named[name] = status
		s.names[status] = name
	}
	var err error
	for _, one := range []struct {
		name string
		into *Status
	}{
		{"ANSWERED", &s.answered},
		{"INJECTION_UNBOUND", &s.unbound},
		{"INJECTION_PROTOCOL_VIOLATION", &s.protocolViolation},
		{"HOST_EXCEPTION", &s.hostException},
	} {
		if *one.into, err = of(one.name); err != nil {
			return statusTable{}, err
		}
	}
	return s, nil
}

// status is the status a manifest gives a name, and whether it does.
func (s statusTable) status(name string) (Status, bool) {
	status, ok := s.named[name]
	return status, ok
}

// Abort is a computation that ended without an answer, for the reason the library numbers.
type Abort struct {
	Status Status
	// Name is what the manifest calls the status, empty where it names none.
	Name string
}

func (e *Abort) Error() string {
	if e.Name == "" {
		return fmt.Sprintf("the library aborted the call with status %d", e.Status)
	}
	return fmt.Sprintf("the library aborted the call: %s (%d)", e.Name, e.Status)
}

var (
	// ErrUnbound is a behavior reached through a requirement it was handed nothing for.
	ErrUnbound = errors.New("a behavior was reached through a requirement it was handed nothing for")
	// ErrProtocolViolation is a host implementation that answered what it may not, or said it
	// failed and nothing was kept.
	ErrProtocolViolation = errors.New("a host implementation answered what it may not, or failed and left nothing")
	// ErrForeignHandle is a value, a function value or what stands for a required behavior that
	// another runtime made, handed to this one's. The call was not made.
	ErrForeignHandle = errors.New("a value that another library's runtime made was handed to this one's")
	// ErrAlreadyRunning is a root run asked for where one of the same runtime is already open on
	// this thread. A run inside it is opened from it with [Run.Scope].
	ErrAlreadyRunning = errors.New("a run of this library is already open on this thread; a run inside it is opened from it with Scope")
)

// HostError is a call that a host implementation it called back answered a failure of its own
// for. Err is the failure the implementation returned.
type HostError struct{ Err error }

func (e *HostError) Error() string { return "a host implementation failed: " + e.Err.Error() }

func (e *HostError) Unwrap() error { return e.Err }

// crossing marks an error that a host implementation's generated code met while crossing what it
// answers, and not one of the implementation's own. It is what the call comes to as it stands.
type crossing struct{ error }

func (e crossing) Unwrap() error { return e.error }

// Crossing marks err as one met while crossing a host implementation's answer, such as text with
// no place as a String or a value another runtime made. The call into the library comes to err
// itself, where any other error an implementation returns comes to a [*HostError].
func Crossing(err error) error { return crossing{err} }

// caught is what a host implementation left for the call it was reached from: a panic to raise
// again, or a failure to answer.
type caught struct {
	panicked bool
	payload  any
	err      error
}

// answered is what a call that answered status comes to, where a host implementation it called
// back left caught: a panic raised again, and a failure answered, whatever the status.
func (s statusTable) outcome(status Status, c *caught) error {
	if c != nil {
		if c.panicked {
			panic(c.payload)
		}
		var crossed crossing
		if errors.As(c.err, &crossed) {
			return crossed.error
		}
		return &HostError{c.err}
	}
	switch status {
	case s.answered:
		return nil
	case s.hostException, s.protocolViolation:
		return ErrProtocolViolation
	case s.unbound:
		return ErrUnbound
	}
	return &Abort{Status: status, Name: s.names[status]}
}

// Misuse is a program that uses a run in a way Rust's types do not let it: a value after its run
// ended, a run from another goroutine, or a run that is not the innermost. It is a fault of the
// program and not a condition to handle, so it is raised as a panic. Err is one of
// [ErrExpired], [ErrRunOnAnotherGoroutine], [ErrNotTheInnermostRun] and [ErrNoValue].
type Misuse struct{ Err error }

func (m *Misuse) Error() string { return m.Err.Error() }

func (m *Misuse) Unwrap() error { return m.Err }

var (
	// ErrExpired is a value used after the run it was made in ended.
	ErrExpired = errors.New("the run this was made in has ended")
	// ErrRunOnAnotherGoroutine is a run, or a value made in one, used from another goroutine than
	// the one that opened it.
	ErrRunOnAnotherGoroutine = errors.New("a run belongs to the goroutine that opened it")
	// ErrNotTheInnermostRun is something made through a run while a run inside it is open.
	ErrNotTheInnermostRun = errors.New("something was made through a run that is not the innermost one")
	// ErrNoValue is a value that was never made: the zero value of a handle.
	ErrNoValue = errors.New("this handle holds no value: it is a zero value, and was not made by the library")
)

func misuse(err error) { panic(&Misuse{err}) }
