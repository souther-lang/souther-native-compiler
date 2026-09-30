package souther.nativecode;

import java.nio.charset.StandardCharsets;
import java.util.ArrayList;
import java.util.LinkedHashMap;
import java.util.List;
import java.util.Map;

/**
 * A C harness that hands documents to a type's decoder and says what each came to, one line each.
 *
 * <p>A host binding's view and nothing more: the decoder and the encoder a module publishes for a
 * type, and the runtime's functions a reading is asked through. What a line says is {@code value}
 * and the value written back by the type's encoder, {@code issues} and each issue as {@code @path
 * code}, then {@code key=} and its message key where that is not its code, then its metadata as the
 * JSON object it is where that says anything; {@code malformed at} and the offset, or
 * {@code status} and a status that is not {@code ANSWERED}.
 */
final class Decoding {

    /** A document handed to the decoder of one type, under a label the line starts with. */
    record Row(String label, String type, String document) {}

    private final Map<String, String> types = new LinkedHashMap<>();
    private final List<Row> rows = new ArrayList<>();
    private final StringBuilder before = new StringBuilder();

    /** The types a row may name, each under the module that declares and publishes it. */
    Decoding type(String module, String name) {
        types.put(name, module);
        return this;
    }

    Decoding row(String label, String type, String document) {
        if (!types.containsKey(type)) {
            throw new IllegalArgumentException(type + " was not named as a type");
        }
        rows.add(new Row(label, type, document));
        return this;
    }

    /** C written into {@code main} before any row runs, with the symbols declared already. */
    Decoding first(String c) {
        before.append(c);
        return this;
    }

    static String symbol(String module, String type, String operation) {
        return Running.hostSymbol(module, type, operation);
    }

    String harness() {
        StringBuilder c = new StringBuilder("""
                #include <stdint.h>
                #include <stdio.h>
                #include <string.h>

                typedef const void *Value;

                extern int64_t souther_scope_open(void);
                extern uint8_t souther_scope_close(int64_t);
                extern int64_t souther_string_length(Value);
                extern const uint8_t *souther_string_bytes(Value);
                extern int32_t souther_decoded_outcome(Value);
                extern Value souther_decoded_value(Value);
                extern int64_t souther_decoded_malformed_at(Value);
                extern int64_t souther_decoded_issue_count(Value);
                extern Value souther_decoded_issue(Value, int64_t);
                extern Value souther_issue_code(Value);
                extern Value souther_issue_message_key(Value);
                extern Value souther_issue_path(Value);
                extern Value souther_issue_meta(Value);

                typedef uint32_t (*Decode)(const uint8_t *, int64_t, Value *);
                typedef Value (*Encode)(Value);

                static void text(Value s) {
                    printf("%.*s", (int) souther_string_length(s), (const char *) souther_string_bytes(s));
                }

                static int same(Value a, Value b) {
                    return souther_string_length(a) == souther_string_length(b)
                        && memcmp(souther_string_bytes(a), souther_string_bytes(b),
                                  (size_t) souther_string_length(a)) == 0;
                }

                static void report(const char *label, Decode decode, Encode encode,
                                   const char *document, int64_t length) {
                    int64_t scope = souther_scope_open();
                    Value read = 0;
                    uint32_t status = decode((const uint8_t *) document, length, &read);
                    printf("%s: ", label);
                    if (status != 0) {
                        printf("status %u\\n", status);
                        souther_scope_close(scope);
                        return;
                    }
                    switch (souther_decoded_outcome(read)) {
                    case 0:
                        printf("value ");
                        text(encode(souther_decoded_value(read)));
                        break;
                    case 1:
                        printf("issues");
                        for (int64_t at = 0; at < souther_decoded_issue_count(read); at++) {
                            Value issue = souther_decoded_issue(read, at);
                            printf(" [@");
                            text(souther_issue_path(issue));
                            printf(" ");
                            text(souther_issue_code(issue));
                            if (!same(souther_issue_code(issue), souther_issue_message_key(issue))) {
                                printf(" key=");
                                text(souther_issue_message_key(issue));
                            }
                            Value meta = souther_issue_meta(issue);
                            if (souther_string_length(meta) != 2) {
                                printf(" ");
                                text(meta);
                            }
                            printf("]");
                        }
                        break;
                    case 2:
                        printf("malformed at %lld", (long long) souther_decoded_malformed_at(read));
                        break;
                    default:
                        printf("an outcome nobody said");
                    }
                    printf("\\n");
                    souther_scope_close(scope);
                }

                """);
        for (Map.Entry<String, String> type : types.entrySet()) {
            c.append("extern uint32_t decode").append(type.getKey())
                    .append("(const uint8_t *, int64_t, Value *) __asm__(\"")
                    .append(symbol(type.getValue(), type.getKey(), "decode")).append("\");\n");
            c.append("extern Value encode").append(type.getKey()).append("(Value) __asm__(\"")
                    .append(symbol(type.getValue(), type.getKey(), "encode")).append("\");\n");
        }
        c.append("\nint main(void) {\n").append(before);
        for (Row row : rows) {
            byte[] bytes = row.document().getBytes(StandardCharsets.UTF_8);
            c.append("    report(").append(literal(row.label().getBytes(StandardCharsets.UTF_8)))
                    .append(", decode").append(row.type()).append(", encode").append(row.type())
                    .append(", ").append(literal(bytes)).append(", ").append(bytes.length)
                    .append(");\n");
        }
        return c.append("    return 0;\n}\n").toString();
    }

    /** Bytes as a C string literal, each one written in octal so nothing in them is read as C. */
    static String literal(byte[] bytes) {
        StringBuilder written = new StringBuilder("\"");
        for (byte b : bytes) {
            written.append(String.format("\\%03o", b & 0xff));
        }
        return written.append('"').toString();
    }
}
