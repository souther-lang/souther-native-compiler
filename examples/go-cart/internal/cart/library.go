package cart

import (
	"path/filepath"
	"runtime"
)

// Library is the path of the shared library bin/build wrote, under the name the platform gives it.
func Library() string {
	_, here, _, _ := runtime.Caller(0)
	name := "libsouther.so"
	if runtime.GOOS == "darwin" {
		name = "libsouther.dylib"
	}
	return filepath.Join(filepath.Dir(here), "..", "..", "build", "native", name)
}
