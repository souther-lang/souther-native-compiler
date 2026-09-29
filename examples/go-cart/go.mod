module example.com/go-cart

go 1.27

require (
	example.com/go-cart/model v0.0.0
	github.com/google/uuid v1.6.0
	github.com/raoh-project/raoh-go v0.0.0-20260929134234-af24f3ce21c6
	github.com/souther-lang/souther-native-compiler/bindings/go/runtime v0.1.0
	modernc.org/sqlite v1.60.1
)

require (
	github.com/dustin/go-humanize v1.0.1 // indirect
	github.com/mattn/go-isatty v0.0.24 // indirect
	github.com/ncruces/go-strftime v1.0.0 // indirect
	github.com/remyoudompheng/bigfft v0.0.0-20230129092748-24d4a6f8daec // indirect
	golang.org/x/sys v0.48.0 // indirect
	modernc.org/libc v1.77.1 // indirect
	modernc.org/mathutil v1.7.1 // indirect
	modernc.org/memory v1.12.1 // indirect
)

// The binding bin/build writes from cart.sou: a package per module of the model, and Load.
replace example.com/go-cart/model => ./build/go

// Until the runtime is published, a binding reaches it from the clone.
replace github.com/souther-lang/souther-native-compiler/bindings/go/runtime => ../../bindings/go/runtime
