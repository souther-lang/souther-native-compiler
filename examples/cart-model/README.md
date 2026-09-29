# cart-model

The model every host's cart example runs: `cart.sou`, the module `com.example.cart.domain`.
`examples/php-cart`, `examples/rust-cart` and `examples/go-cart` each build it into a library and a
binding of their own language, and nothing of it is written again in any of them.

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

What the model states is whether a value holds, never how it is written: an invariant decides and
does not rewrite. So the canonical form of a client's text stays each host's boundary, as it is the
Java one's: a name and a company name without the spaces around them, an email trimmed and in lower
case, an id in lower case. A host hands the model the canonical text, and the bounds above are on
that.

A rule the model states is reported by the model's decoder at the field's path. A newtype's rule
with a Raoh equivalent is reported with that constraint's code and message key (`too_long`,
`invalid_format`), by a native library as on the JVM; any other rule is `invariant_violation`.
