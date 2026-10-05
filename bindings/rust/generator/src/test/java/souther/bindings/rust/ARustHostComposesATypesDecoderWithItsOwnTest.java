package souther.bindings.rust;

import souther.bindings.testkit.SoutherBindingTest;
import souther.bindings.testkit.TestLibrary;
import org.junit.jupiter.api.Test;
import org.junit.jupiter.api.io.TempDir;
import souther.nativecode.Generated;



import java.nio.file.Path;
import java.util.List;

import static org.assertj.core.api.Assertions.assertThat;

/**
 * A Rust host composes a type's decoder with raoh's own, as a JVM host composes a type's
 * {@code decoder()}: raoh checks the form of what came in, and the model's decoder reads what it
 * handed on as the type, checking what the type states and telling a sum's cases apart. A
 * constructor is made into a decoder the same way, through the run a {@code Decoding} lends.
 *
 * <p>What the model finds wrong is an issue at the path the decoder was reached at, beside raoh's
 * own, so a host answers both the same way.
 */
class ARustHostComposesATypesDecoderWithItsOwnTest {

    private static final String ORDERING = """
            module ordering exposing ( Email, Individual, Corporation, Orderer, Quantity, Line, Lines,
                                       Free, Voucher, Priced )

            data Email = String
                invariant String.length(value) >= 3
            data Individual = { email: Email, name: String }
            data Corporation = { email: Email, companyName: String }
            data Orderer = Individual | Corporation

            data Quantity = Int
                invariant value > 0
            data Line = { sku: String, quantity: Quantity }
            data Lines = { lines: List<Line> }

            data Free
            data Voucher = { note: String, perk: Free }

            data Priced = { amount: Decimal, on: Date }
            """;

    private static final String HOST = """
            use shop_binding::ordering::{Email, Individual, Lines, Orderer, OrdererCase, Priced, Quantity, Voucher};
            use shop_binding::raoh::json::prelude::*;
            use shop_binding::raoh::{Decoder, Issues};
            use shop_binding::{Decoding, Library, Run};

            fn said<T>(read: Result<Result<T, Issues>, shop_binding::Failure>) -> String {
                match read {
                    Ok(Ok(_)) => "ok".to_owned(),
                    Ok(Err(issues)) => issues
                        .iter()
                        .map(|it| {
                            let path = it.path().to_string();
                            let key = if it.message_key() == it.code() {
                                String::new()
                            } else {
                                format!(" key={}", it.message_key())
                            };
                            format!("[{} {}{}]", if path.is_empty() { "/" } else { &path }, it.code(), key)
                        })
                        .collect::<Vec<_>>()
                        .join(" "),
                    Err(failure) => format!("failed: {failure}"),
                }
            }

            fn checkout(run: &mut Run<'_>, text: &str) -> String {
                said(Decoding::read(run, |decoding| {
                    from_str(&object((field("orderer", Orderer::decoder(decoding)),)), text)
                }))
            }

            fn main() {
                let path = std::env::args().nth(1).expect("the library's path");
                // SAFETY: the library the binding was generated from.
                let library = unsafe { Library::load(&path) }.expect("the library loads");
                library
                    .run(|run| {
                        println!("individual: {}", checkout(run, r#"{"orderer":{"type":"Individual","email":"a@b","name":"Taro"}}"#));
                        let read = Decoding::read(run, |decoding| {
                            from_str(&object((field("orderer", Orderer::decoder(decoding)),)),
                                r#"{"orderer":{"type":"Individual","email":"a@b","name":"Taro"}}"#)
                        });
                        let (orderer,) = read.unwrap().unwrap();
                        if let OrdererCase::Individual(it) = orderer.case() {
                            println!("read: {} {}", it.email().value(), it.name());
                        }
                        println!("corporation: {}", checkout(run, r#"{"orderer":{"type":"Corporation","email":"x@y","companyName":"Acme"}}"#));
                        println!("short email: {}", checkout(run, r#"{"orderer":{"type":"Individual","email":"ab","name":"Taro"}}"#));
                        println!("no such case: {}", checkout(run, r#"{"orderer":{"type":"Robot"}}"#));
                        println!("missing field: {}", checkout(run, r#"{"orderer":{"type":"Corporation","email":"x@y"}}"#));
                        println!("missing member: {}", checkout(run, "{}"));

                        // A field of the host's own beside one of the model's, and a constructor made
                        // into a decoder after the host's: each issue is at its own path.
                        println!("both: {}", said(Decoding::read(run, |decoding| {
                            object((
                                field("who", string().min_length(2)),
                                field("individual", Individual::decoder(decoding)),
                                field("email", string().trim().pipe(decoding.of(|run, it: &String| Email::new(run, it)))),
                            ))
                            .decode(&json!({"who": "x", "individual": {"email": "ab", "name": "T"}, "email": " yz "}))
                        })));

                        println!("quantity: {}", said(Decoding::read(run, |decoding| {
                            object((field("n", Quantity::decoder(decoding)),)).decode(&json!({"n": 0}))
                        })));
                        println!("a float: {}", said(Decoding::read(run, |decoding| {
                            from_str(&Quantity::decoder(decoding), "2.0")
                        })));
                        println!("lines: {}", said(Decoding::read(run, |decoding| {
                            Lines::decoder(decoding).decode(&json!({"lines": [{"sku": "a", "quantity": 1}, {"sku": "b", "quantity": 0}]}))
                        })));
                        println!("a unit in a product: {}", said(Decoding::read(run, |decoding| {
                            Voucher::decoder(decoding).decode(&json!({"note": "x", "perk": {}}))
                        })));

                        // A number reaches the library as it was spelt: its scale, and digits no
                        // f64 holds.
                        for text in [
                            r#"{"amount": 1.2300, "on": "2026-09-29"}"#,
                            r#"{"amount": 12345678901234567890.123456789, "on": "2026-09-29"}"#,
                            r#"{"amount": -0.10, "on": "2026-09-29"}"#,
                        ] {
                            let read = Decoding::read(run, |decoding| from_str(&Priced::decoder(decoding), text));
                            let priced = read.unwrap().unwrap();
                            let amount = priced.amount();
                            let on = priced.on();
                            println!("priced: {} scale {} on {}-{}-{}", amount.unscaled(), amount.scale(),
                                on.year(), on.month(), on.day());
                        }
                    })
                    .unwrap();
            }
            """;

    @Test
    void aTypesDecoderComposesWithRaohsAndReportsAtItsPath(@TempDir Path into) throws Exception {
        TestLibrary library =
                SoutherBindingTest.compile(into.resolve("native"), ORDERING);
        Generated binding = RustHost.generated(library, into.resolve("binding"), "shop-binding");

        String said = RustHost.ran(into, binding, "shop-binding", HOST,
                List.of(library.library().toString()));

        assertThat(said).isEqualTo("""
                individual: ok
                read: a@b Taro
                corporation: ok
                short email: [/orderer/email too_short]
                no such case: [/orderer/type not_allowed]
                missing field: [/orderer/companyName required]
                missing member: [/orderer required]
                both: [/who too_short] [/individual/email too_short] [/email invariant_violation]
                quantity: [/n out_of_range key=out_of_range.positive]
                a float: [/ type_mismatch]
                lines: [/lines/1/quantity out_of_range key=out_of_range.positive]
                a unit in a product: ok
                priced: 12300 scale 4 on 2026-9-29
                priced: 12345678901234567890123456789 scale 9 on 2026-9-29
                priced: -10 scale 2 on 2026-9-29
                """);
    }
}
