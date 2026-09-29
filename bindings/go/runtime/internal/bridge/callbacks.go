package bridge

/*
#include <stdint.h>
*/
import "C"

import (
	"unsafe"

	souther "github.com/souther-lang/souther-native-compiler/bindings/go/runtime"
)

// A file that exports a function has a preamble of declarations only: it is copied into two C
// files, and a definition there is one twice. What is defined is in bridge.go.

// bridgeImplementation is what the library calls for the behavior the host implements: it runs
// the implementation in the run its call was made in, and answers the status that says whether it
// answered or failed.
//
//export bridgeImplementation
func bridgeImplementation(userdata unsafe.Pointer, x C.int64_t, out *C.int64_t) C.uint32_t {
	hosted := souther.UserdataValue(userdata).(*hostedImplementation)
	return C.uint32_t(souther.Host(hosted.origin, func(in *Run) error {
		answer, err := hosted.impl.Apply(in, int64(x))
		if err != nil {
			return err
		}
		*out = C.int64_t(answer)
		return nil
	}))
}
