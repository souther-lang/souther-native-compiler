package cart

import (
	"path/filepath"
	"runtime"
)

// Library is the path of the shared library bin/build wrote into dir, under the name the platform
// gives it.
func Library(dir string) string {
	name := "libsouther.so"
	if runtime.GOOS == "darwin" {
		name = "libsouther.dylib"
	}
	return filepath.Join(dir, name)
}
