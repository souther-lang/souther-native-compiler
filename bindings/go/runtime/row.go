package souther

import (
	"fmt"
	"strconv"
	"unsafe"
)

// RowIdentity is what a row of a behavior names itself: the name it was written with, or which row
// it is, from one, where it was written with none.
type RowIdentity struct {
	name    string
	ordinal int
}

// RowNamed is the identity of a row written with name.
func RowNamed(name string) RowIdentity { return RowIdentity{name: name} }

// RowUnnamed is the identity of a row written with no name, the ordinal-th of the behavior's rows.
func RowUnnamed(ordinal int) RowIdentity { return RowIdentity{ordinal: ordinal} }

// Name is the name the row was written with, and whether it was written with one.
func (id RowIdentity) Name() (string, bool) { return id.name, id.ordinal == 0 }

// Shown is what a report calls the row: its name, or "row n" for the n-th where it has none, as
// every runtime and the JVM call it.
func (id RowIdentity) Shown() string {
	if id.ordinal == 0 {
		return id.name
	}
	return "row " + strconv.Itoa(id.ordinal)
}

// RowKind is what a host can do with a row: run it and be told whether the answer keeps it, run it
// and be told the answer only, or nothing.
type RowKind int

const (
	// Asserted is a row that states an answer, which the library holds what an implementation
	// answers to by the language's equality.
	Asserted RowKind = iota
	// Owed is a row whose answer is owed (<?>): it states nothing an answer could keep, so running
	// it says what an implementation answers and nothing of whether that is right.
	Owed
	// NotComparable is a row that states an answer the library holds no value of what the behavior
	// answers to compare with, so there is nothing to run.
	NotComparable
	// Unreproducible is a row the compile could not read, so there is nothing to run.
	Unreproducible
)

func (k RowKind) String() string {
	switch k {
	case Asserted:
		return "asserted"
	case Owed:
		return "owed"
	case NotComparable:
		return "not comparable"
	case Unreproducible:
		return "unreproducible"
	}
	return "RowKind(" + strconv.Itoa(int(k)) + ")"
}

// notComparable is why a row the library cannot compare an answer with has nothing to run.
const notComparable = "the library holds no value of what the behavior answers that the row states"

// Row is one row of a behavior a host implements, of the behavior I is the interface of, which a
// host runs against an implementation of its own with [Row.Evaluate]. A generated package lists the
// rows of each such behavior in the order they are written (<Behavior>Examples), and a row of one
// behavior is another type than a row of another.
//
// What is listed and what is run are two things, as in the JVM's BoundExamples, and nothing runs
// every row at once: what an implementation answers from (a database, a clock) is the state of the
// test that loops over the rows, which makes it for each row as it would for any other call.
type Row[B, I any] struct {
	at        int
	identity  RowIdentity
	kind      RowKind
	why       string
	implement func(*Run[B], I) Capability[B]
	call      func(r *Run[B], capabilities unsafe.Pointer, kept *uint8, answer *unsafe.Pointer) Status
}

// Rows is what a generated package makes the rows of one behavior with: how an implementation of
// it is made into its capability, which every row of it is run with.
type Rows[B, I any] struct {
	implement func(*Run[B], I) Capability[B]
}

// RowsOf makes the rows of a behavior whose implementations implement makes into its capability,
// as Implement<Behavior> does.
func RowsOf[B, I any](implement func(*Run[B], I) Capability[B]) Rows[B, I] {
	return Rows[B, I]{implement}
}

// Asserted is the row at among the behavior's rows that states an answer: call calls the library's
// function for it with the capabilities the row runs with, room for whether the answer keeps the
// row and room for the answer in external form, and answers its status.
func (rs Rows[B, I]) Asserted(at int, identity RowIdentity,
	call func(r *Run[B], capabilities unsafe.Pointer, kept *uint8, answer *unsafe.Pointer) Status) Row[B, I] {
	return Row[B, I]{at: at, identity: identity, kind: Asserted, implement: rs.implement, call: call}
}

// Owed is the row at among the behavior's rows whose answer is owed: as [Rows.Asserted], and call
// is handed no room for whether the answer keeps the row, which nothing writes.
func (rs Rows[B, I]) Owed(at int, identity RowIdentity,
	call func(r *Run[B], capabilities unsafe.Pointer, answer *unsafe.Pointer) Status) Row[B, I] {
	return Row[B, I]{at: at, identity: identity, kind: Owed, implement: rs.implement,
		call: func(r *Run[B], capabilities unsafe.Pointer, _ *uint8, answer *unsafe.Pointer) Status {
			return call(r, capabilities, answer)
		}}
}

// NotComparable is the row at among the behavior's rows that states an answer the library cannot
// compare one with, which has nothing to run.
func (rs Rows[B, I]) NotComparable(at int, identity RowIdentity) Row[B, I] {
	return Row[B, I]{at: at, identity: identity, kind: NotComparable, why: notComparable}
}

// Unreproducible is the row at among the behavior's rows that the compile could not read, for the
// reason why, which has nothing to run.
func (rs Rows[B, I]) Unreproducible(at int, identity RowIdentity, why string) Row[B, I] {
	return Row[B, I]{at: at, identity: identity, kind: Unreproducible, why: why}
}

// At is where the row stands among the behavior's rows, from nought.
func (row Row[B, I]) At() int { return row.at }

// Identity is what the row names itself.
func (row Row[B, I]) Identity() RowIdentity { return row.identity }

// Shown is what a report calls the row, which is a name a subtest of it can be run under.
func (row Row[B, I]) Shown() string { return row.identity.Shown() }

// Kind is what a host can do with the row.
func (row Row[B, I]) Kind() RowKind { return row.kind }

// Why is why the row has nothing to run, empty where it has something.
func (row Row[B, I]) Why() string { return row.why }

// Evaluate runs the row in r against impl: impl is made into the behavior's capability in r, as
// Implement<Behavior> makes one, and the row is run with it, which hands it what the row states
// and has the library hold the answer to it.
//
// Whether an answer keeps the row is the library's to say, by the language's equality, and not the
// external form's: a decimal at another scale, or a set in another order, keeps a row its text does
// not match. The external form is only what a report shows.
//
// An implementation's error, and anything else that ends the row with no answer, comes to an
// outcome that ended, with the error a call of the behavior would come to; a panic in impl is
// raised again here, as a call raises it. A row with nothing to run is not run, and impl is not
// called. It panics with a [*Misuse] where something may not be made through r. What is made is
// held until r ends, so a test that evaluates many rows in one run may evaluate each in a
// [Run.Scope] of it.
func (row Row[B, I]) Evaluate(r *Run[B], impl I) RowOutcome {
	r.checkMaking()
	if row.call == nil {
		return RowOutcome{disposition: NotRun, why: row.why}
	}
	// The row's function is handed the capabilities of what it runs, the behavior's own the only
	// one, as a behavior bound to it would be.
	capabilities, err := Bound(r, []Capability[B]{row.implement(r, impl)}, nil).Requirements(r)
	if err != nil {
		return RowOutcome{disposition: Ended, err: err}
	}
	// Nought or one is what the library writes, and an answer always: what it leaves as it was is
	// no verdict, and no answer.
	kept := uint8(2)
	var answer unsafe.Pointer
	if err := Called(r, func() Status { return row.call(r, capabilities, &kept, &answer) }); err != nil {
		return RowOutcome{disposition: Ended, err: err}
	}
	if answer == nil || row.kind != Owed && kept > 1 {
		return RowOutcome{disposition: Ended, err: ErrProtocolViolation}
	}
	written := Text(r, answer)
	switch {
	case row.kind == Owed:
		return RowOutcome{disposition: Answered, answer: written}
	case kept == 1:
		return RowOutcome{disposition: Kept, answer: written}
	}
	return RowOutcome{disposition: NotKept, answer: written}
}

// Disposition is what came of evaluating a row.
type Disposition int

const (
	// NotRun is a row with nothing to run: one the compile could not read, or one stating an
	// answer the library cannot compare one with.
	NotRun Disposition = iota
	// Kept is an answer that keeps the row.
	Kept
	// NotKept is an answer that does not keep the row.
	NotKept
	// Answered is an answer to a row whose answer is owed, of which nothing is said but what it is.
	Answered
	// Ended is a row that ended with no answer: the implementation failed, or the library ended
	// the computation for a reason of its own, such as a value that holds no invariant of its type.
	Ended
)

func (d Disposition) String() string {
	switch d {
	case NotRun:
		return "not run"
	case Kept:
		return "kept"
	case NotKept:
		return "not kept"
	case Answered:
		return "answered"
	case Ended:
		return "ended"
	}
	return "Disposition(" + strconv.Itoa(int(d)) + ")"
}

// RowOutcome is what came of evaluating a row against an implementation: its [Disposition], and
// what goes with it. An answer is in the language's external form, for a report, and is nothing to
// compare: whether it kept the row is the disposition.
type RowOutcome struct {
	disposition Disposition
	answer      string
	err         error
	why         string
}

// Disposition is what came of the evaluation.
func (o RowOutcome) Disposition() Disposition { return o.disposition }

// Kept is whether the answer kept the row. An owed row's answer keeps nothing, and is not a
// failure either: a test asks [RowOutcome.Failed] for that.
func (o RowOutcome) Kept() bool { return o.disposition == Kept }

// Failed is whether the row was run and its answer did not keep it, or it ended with no answer.
// A row with nothing to run has not failed, and nor has an owed row that was answered.
func (o RowOutcome) Failed() bool { return o.disposition == NotKept || o.disposition == Ended }

// Answer is the answer in the language's external form, and whether there is one: there is for a
// row that was kept, not kept or answered.
func (o RowOutcome) Answer() (string, bool) {
	switch o.disposition {
	case Kept, NotKept, Answered:
		return o.answer, true
	}
	return "", false
}

// Err is what a row that ended came to, as a call of the behavior would: a [*HostError] where the
// implementation returned one, an [*Abort] where the library ended the computation for a reason
// of its own. It is nil for any other disposition.
func (o RowOutcome) Err() error { return o.err }

// Why is why a row that was not run has nothing to run, empty for any other disposition.
func (o RowOutcome) Why() string { return o.why }

// String says what came of the evaluation in one line, for a report.
func (o RowOutcome) String() string {
	switch o.disposition {
	case Kept, NotKept:
		return fmt.Sprintf("%s, answered %s", o.disposition, o.answer)
	case Answered:
		return fmt.Sprintf("%s %s", o.disposition, o.answer)
	case Ended:
		return fmt.Sprintf("%s: %v", o.disposition, o.err)
	}
	return fmt.Sprintf("%s: %s", o.disposition, o.why)
}
