package souther

import (
	"sync"
	"testing"

	"github.com/raoh-project/raoh-go"
)

// read is a type's reading as Decoder takes one, answering the text it was handed.
func read(*Run[tagA], []byte) (string, error) { return "read", nil }

func TestADecoderReadsInTheRunItWasMadeIn(t *testing.T) {
	lib, _ := libraryOf[tagA](t, 1)
	_ = lib.Run(func(r *Run[tagA]) error {
		if got, err := Decoder(r, read).Decode(raoh.JSONObject{}); err != nil || got != "read" {
			t.Errorf("got %q, %v", got, err)
		}
		return nil
	})
}

func TestADecoderKeptPastItsRunIsExpired(t *testing.T) {
	lib, _ := libraryOf[tagA](t, 1)
	var kept raoh.Decoder[any, string]
	_ = lib.Run(func(r *Run[tagA]) error {
		kept = Decoder(r, read)
		return nil
	})
	misused(t, ErrExpired, func() { _, _ = kept.Decode(raoh.JSONObject{}) })
	// Missing input is not read, and the decoder is expired all the same.
	misused(t, ErrExpired, func() { _, _ = kept.Decode(nil) })
}

func TestADecoderSharedWithAnotherGoroutineIsRefusedThere(t *testing.T) {
	lib, _ := libraryOf[tagA](t, 1)
	_ = lib.Run(func(r *Run[tagA]) error {
		shared := Decoder(r, read)
		var wg sync.WaitGroup
		wg.Add(1)
		go func() {
			defer wg.Done()
			misused(t, ErrRunOnAnotherGoroutine, func() { _, _ = shared.Decode(raoh.JSONObject{}) })
		}()
		wg.Wait()
		return nil
	})
}
