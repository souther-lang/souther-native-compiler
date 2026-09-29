// What Rust refuses to compile, and Go refuses where it happens. A value of the model lives in an
// arena that belongs to the run it was made in, and the run ends with the request. rust-cart holds
// each mistake beside what rustc says about it; here each is a panic with a *souther.Misuse at the
// first use, which is a fault of the program and not a condition a handler answers.
package cart_test

import (
	"errors"
	"testing"

	souther "github.com/souther-lang/souther-native-compiler/bindings/go/runtime"

	"example.com/go-cart/internal/cart"
	"example.com/go-cart/internal/store"
	"example.com/go-cart/model"
	"example.com/go-cart/model/com/example/cart/domain"
)

func load(t *testing.T) *model.Library {
	t.Helper()
	library, err := model.Load(cart.Library("../../build/native"))
	if err != nil {
		t.Fatalf("run bin/build first: %v", err)
	}
	return library
}

// misuse is the misuse use panicked with, or nil where it did not.
func misuse(use func()) (found error) {
	defer func() {
		if p := recover(); p != nil {
			m, ok := p.(*souther.Misuse)
			if !ok {
				panic(p)
			}
			found = m.Err
		}
	}()
	use()
	return nil
}

func TestAValueKeptPastItsRequestIsNeverRead(t *testing.T) {
	// As a cache would keep one.
	var kept domain.UserId
	err := load(t).Run(func(r *model.Run) error {
		var err error
		kept, err = domain.NewUserId(r, "11111111-1111-1111-1111-111111111111")
		return err
	})
	if err != nil {
		t.Fatal(err)
	}

	if got := misuse(func() { kept.Value() }); !errors.Is(got, souther.ErrExpired) {
		t.Fatalf("reading it came to %v", got)
	}
}

func TestAValueHandedToAnotherGoroutineIsNeverRead(t *testing.T) {
	err := load(t).Run(func(r *model.Run) error {
		id, err := domain.NewUserId(r, "11111111-1111-1111-1111-111111111111")
		if err != nil {
			return err
		}
		found := make(chan error)
		go func() { found <- misuse(func() { id.Value() }) }()
		if got := <-found; !errors.Is(got, souther.ErrRunOnAnotherGoroutine) {
			t.Errorf("reading it came to %v", got)
		}
		return nil
	})
	if err != nil {
		t.Fatal(err)
	}
}

func TestABehaviorKeptPastItsRequestIsNeverCalled(t *testing.T) {
	// A bound behavior is held until the run it was bound in ends, and with it the implementations,
	// which hold the request's transaction: none of them is reached once it is committed.
	library := load(t)
	var kept domain.IssueQuoteBound
	err := library.Run(func(r *model.Run) error {
		kept = domain.BindIssueQuote(r, domain.ImplementPriceCart(r, store.PriceCart{}))
		return nil
	})
	if err != nil {
		t.Fatal(err)
	}

	err = library.Run(func(r *model.Run) error {
		quoteID, err := domain.NewQuoteId(r, "q")
		if err != nil {
			return err
		}
		if got := misuse(func() { _, _ = kept.Call(r, quoteID, domain.UserId{}, domain.Corporation{}, "") }); !errors.Is(got, souther.ErrExpired) {
			t.Errorf("calling it came to %v", got)
		}
		return nil
	})
	if err != nil {
		t.Fatal(err)
	}
}
