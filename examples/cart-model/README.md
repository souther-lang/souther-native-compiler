# cart-model

The model every host's cart example runs: `cart.sou`, the module `com.example.cart.domain`.
`examples/php-cart` and `examples/rust-cart` each build it into a library and a binding of their own
language, and nothing of it is written again in either.

It is the `cart.sou` of the Java example,
[`boundaries-not-layers/examples/raoh-souther`](https://github.com/kawasima/boundaries-not-layers/tree/main/examples/raoh-souther),
with two changes.

The discount is `sub * 10 / 100` there. Since `/` answers the exact quotient, a `Rational`, which
this backend has no representation of, it is said as `Int.truncatingDivide(sub * 10, 100)`,
matching its `DivisionByZero` case, which the divisor of 100 never takes.

What an orderer's fields are is stated here, where the Java example leaves it to the boundary. A
name is a `PersonName` and a company's a `CompanyName`, each not blank and at most 100 and 200 code
points long, and a corporate number is a `CorporateNumber` of thirteen digits. The Java boundary
checks these with raoh before the model's decoder reads the orderer; here the model's decoder
checks them itself, so no host says them again. What the model still leaves to a boundary is what
it says it does: an email's shape, and that an id is a UUID.

A rule the model states is reported by the model's decoder at the field's path. On the JVM a
newtype's rule with a Raoh equivalent is reported with that constraint's code (`too_long`,
`invalid_format`); a native library reports every one as `invariant_violation` for now (#97).
