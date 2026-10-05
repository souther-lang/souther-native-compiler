package souther.bindings.rust;

import org.junit.jupiter.api.Test;
import org.junit.jupiter.api.io.TempDir;
import souther.bindings.testkit.SoutherBindingTest;
import souther.bindings.testkit.TestLibrary;
import souther.nativecode.Generated;
import souther.nativecode.IssueMetaContract;

import java.nio.file.Path;
import java.util.List;
import java.util.StringJoiner;

import static org.assertj.core.api.Assertions.assertThat;

/**
 * A Rust host is handed each value of an issue's metadata as the type raoh holds it, as the JVM's
 * decoder holds it: what the host reads off its {@code raoh::Issue} is held to what the JVM
 * answers ({@link IssueMetaContract}), and holds a value of every type the library writes. The
 * library writing each value as its type is held elsewhere; this is the other half, the runtime
 * making it a value of raoh's, where a type the runtime did not keep would be lost to the host.
 */
class ARustHostIsHandedEachValueOfAnIssueAsItsTypeTest {

    private static String host() {
        StringJoiner rows = new StringJoiner("\n");
        for (IssueMetaContract.Row row : IssueMetaContract.ROWS) {
            rows.add("        println!(\"%s: {}\", said(Decoding::read(run, |decoding| from_str(&%s::decoder(decoding), %s))));"
                    .formatted(row.label(), row.type(), rust(row.document())));
        }
        return """
                use shop_binding::meta::*;
                use shop_binding::raoh::json::prelude::*;
                use shop_binding::raoh::{Issues, MetaValue};
                use shop_binding::{Decoding, Library};

                fn written(value: &MetaValue) -> String {
                    match value {
                        MetaValue::Int(n) => format!("int:{n}"),
                        MetaValue::Decimal(d) => format!("decimal:{d}"),
                        MetaValue::String(s) => format!("string:{}", Value::String(s.clone())),
                        MetaValue::Bool(b) => format!("bool:{b}"),
                        MetaValue::Date(d) => format!("date:{d}"),
                        MetaValue::Time(t) => format!("time:{t}"),
                        MetaValue::DateTime(t) => format!("datetime:{t}"),
                        MetaValue::Instant(t) => format!("instant:{t}"),
                        MetaValue::List(items) => format!(
                            "list:[{}]",
                            items.iter().map(written).collect::<Vec<_>>().join(",")
                        ),
                        other => panic!("metadata of no type the library writes: {other:?}"),
                    }
                }

                fn said<T>(read: Result<Result<T, Issues>, shop_binding::Failure>) -> String {
                    match read {
                        Ok(Ok(_)) => "ok".to_owned(),
                        Ok(Err(issues)) => issues
                            .iter()
                            .map(|it| {
                                let key = if it.message_key() == it.code() {
                                    String::new()
                                } else {
                                    format!(" key={}", it.message_key())
                                };
                                let meta = it
                                    .meta()
                                    .iter()
                                    .map(|(name, value)| format!("{}:{}", Value::String(name.clone()), written(value)))
                                    .collect::<Vec<_>>()
                                    .join(",");
                                format!("[{} {}{} {{{}}}]", it.path(), it.code(), key, meta)
                            })
                            .collect::<Vec<_>>()
                            .join(" "),
                        Err(failure) => format!("failed: {failure}"),
                    }
                }

                fn main() {
                    let path = std::env::args().nth(1).expect("the library's path");
                    // SAFETY: the library the binding was generated from.
                    let library = unsafe { Library::load(&path) }.expect("the library loads");
                    library
                        .run(|run| {
                %s
                        })
                        .unwrap();
                }
                """.formatted(rows);
    }

    /** A document as a Rust raw string. */
    private static String rust(String document) {
        return "r#\"" + document + "\"#";
    }

    @Test
    void eachValueIsTheTypeTheJvmHoldsItAs(@TempDir Path into) throws Exception {
        TestLibrary library = SoutherBindingTest.compile(into.resolve("native"), IssueMetaContract.MODULE);
        Generated binding = RustHost.generated(library, into.resolve("binding"), "shop-binding");

        String said = RustHost.ran(into, binding, "shop-binding", host(),
                List.of(library.library().toString()));

        assertThat(said).isEqualTo(IssueMetaContract.expected());
        assertThat(IssueMetaContract.typesIn(said)).isEqualTo(IssueMetaContract.metaTypes());
    }
}
