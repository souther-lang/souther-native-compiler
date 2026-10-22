// What evaluating a row comes to where the library is a stand-in: a row with nothing to run, and a
// row whose function answers no verdict. What an answer is held to is the library's, and is held
// end to end by the generator's tests.
package souther

import (
	"errors"
	"testing"
	"unsafe"
)

// implementation is what a row of the stand-in is run against.
type implementation struct{}

// rowsOf are rows whose capability is one the stand-in never reads.
func rowsOf(called *int) Rows[tagA, implementation] {
	return RowsOf(func(r *Run[tagA], _ implementation) Capability[tagA] {
		*called++
		return Capability[tagA]{run: r}
	})
}

func evaluated(t *testing.T, row Row[tagA, implementation]) RowOutcome {
	t.Helper()
	lib, _ := libraryOf[tagA](t, 1)
	lib.layout = Layout{Pointer: unsafe.Sizeof(unsafe.Pointer(nil)), Capability: 1, Hosted: 1, HostedFunction: 1}
	var outcome RowOutcome
	if err := lib.Run(func(r *Run[tagA]) error {
		outcome = row.Evaluate(r, implementation{})
		return nil
	}); err != nil {
		t.Fatal(err)
	}
	return outcome
}

func TestARowWithNothingToRunIsNotRunAndHasNotFailed(t *testing.T) {
	called := 0
	rows := rowsOf(&called)
	for _, row := range []Row[tagA, implementation]{
		rows.Unreproducible(0, RowNamed("unread"), "the compile could not read it"),
		rows.NotComparable(1, RowUnnamed(2)),
	} {
		outcome := evaluated(t, row)
		if outcome.Disposition() != NotRun || outcome.Failed() || outcome.Kept() || outcome.Why() != row.Why() {
			t.Errorf("%s: %v", row.Shown(), outcome)
		}
		if _, ok := outcome.Answer(); ok {
			t.Errorf("%s has an answer", row.Shown())
		}
	}
	if called != 0 {
		t.Errorf("an implementation was made %d times for rows with nothing to run", called)
	}
}

func TestARowEndedByTheLibraryHasTheErrorACallWouldHave(t *testing.T) {
	called := 0
	row := rowsOf(&called).Asserted(0, RowNamed("divided"),
		func(*Run[tagA], unsafe.Pointer, *uint8, *unsafe.Pointer) Status { return divisionByZero })
	outcome := evaluated(t, row)
	var abort *Abort
	if outcome.Disposition() != Ended || !outcome.Failed() || !errors.As(outcome.Err(), &abort) ||
		abort.Status != divisionByZero || called != 1 {
		t.Fatalf("%v, made %d times", outcome, called)
	}
}

func TestARowAnsweredWithNoVerdictWrittenIsAViolation(t *testing.T) {
	called := 0
	row := rowsOf(&called).Asserted(0, RowUnnamed(1),
		func(_ *Run[tagA], _ unsafe.Pointer, _ *uint8, answer *unsafe.Pointer) Status {
			*answer = unsafe.Pointer(&called)
			return answered
		})
	if outcome := evaluated(t, row); outcome.Disposition() != Ended || !errors.Is(outcome.Err(), ErrProtocolViolation) {
		t.Fatalf("%v", outcome)
	}
}

func TestARowIsShownByItsNameOrItsPlace(t *testing.T) {
	if name, named := RowNamed("three").Name(); !named || name != "three" || RowNamed("three").Shown() != "three" {
		t.Errorf("a named row is %q", RowNamed("three").Shown())
	}
	if _, named := RowUnnamed(2).Name(); named || RowUnnamed(2).Shown() != "row 2" {
		t.Errorf("an unnamed row is %q", RowUnnamed(2).Shown())
	}
}
