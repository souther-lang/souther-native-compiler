package souther.nativecode;

import org.junit.jupiter.api.Test;
import org.junit.jupiter.api.io.TempDir;

import java.io.IOException;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.Path;
import java.util.List;

import static org.assertj.core.api.Assertions.assertThat;

/**
 * A host's construction answers as the type's decoder does for the same value, through the header
 * alone: a clause that does not hold is a reading holding the issue the decoder records, its code,
 * message key, path and metadata each the same, numbers among them; and a clause that itself ends
 * without a value is the status it ended with, with nothing written, which is not a reading of any
 * issue. The two are told apart by the status, as they are for a decoder.
 */
class AHostsConstructionAnswersWhatItsDecoderAnswersTest {

    private static final String HELD = """
            module held exposing ( Ratio, Name )

            data Ratio = { n: Int, d: Int }
                invariant summed = n + d >= 0

            data Name = String
                invariant long = String.length(value) >= 3
            """;

    private static final String HARNESS = """
            #include <inttypes.h>
            #include <stdio.h>
            #include <string.h>
            #include "souther.h"

            static void text(souther_string said) {
                printf("%.*s", (int) souther_string_length(said),
                       (const char *) souther_string_bytes(said));
            }

            /* What a reading came to: its value, or each issue as everything a host is told of it. */
            static void said(const char *label, souther_status status, souther_decoded reading) {
                printf("%s: status %u,", label, status);
                if (souther_decoded_outcome(reading) == SOUTHER_DECODED_VALUE) {
                    printf(" a value\\n");
                    return;
                }
                for (int64_t at = 0; at < souther_decoded_issue_count(reading); at++) {
                    souther_issue issue = souther_decoded_issue(reading, at);
                    printf(" [");
                    text(souther_issue_path(issue));
                    printf(" ");
                    text(souther_issue_code(issue));
                    printf(" ");
                    text(souther_issue_message_key(issue));
                    printf(" ");
                    text(souther_issue_meta(issue));
                    printf("]");
                }
                printf("\\n");
            }

            static souther_status decodedName(const char *json, souther_decoded *reading) {
                return souther@_m_held_t_Name_decode((const uint8_t *) json, (int64_t) strlen(json),
                                                     reading);
            }

            static souther_status decodedRatio(const char *json, souther_decoded *reading) {
                return souther@_m_held_t_Ratio_decode((const uint8_t *) json, (int64_t) strlen(json),
                                                      reading);
            }

            int main(void) {
                int64_t scope = souther_scope_open();

                souther_string ab = NULL;
                souther_string_of_utf8((const uint8_t *) "ab", 2, &ab);
                souther_decoded reading = NULL;
                souther_status status = souther@_m_held_t_Name_construct(ab, &reading);
                said("name built", status, reading);
                status = decodedName("\\"ab\\"", &reading);
                said("name read", status, reading);

                status = souther@_m_held_t_Ratio_construct(1, -5, &reading);
                said("ratio built", status, reading);
                status = decodedRatio("{\\"n\\":1,\\"d\\":-5}", &reading);
                said("ratio read", status, reading);

                status = souther@_m_held_t_Ratio_construct(2, 3, &reading);
                said("whole built", status, reading);

                souther_decoded untouched = NULL;
                status = souther@_m_held_t_Ratio_construct(INT64_MAX, 1, &untouched);
                printf("ended built: %d, nothing written %d\\n",
                       status == SOUTHER_REQUIRED_FORM_HAS_NO_PLACE, untouched == NULL);
                untouched = NULL;
                status = decodedRatio("{\\"n\\":9223372036854775807,\\"d\\":1}", &untouched);
                printf("ended read: %d, nothing written %d\\n",
                       status == SOUTHER_REQUIRED_FORM_HAS_NO_PLACE, untouched == NULL);

                souther_scope_close(scope);
                return 0;
            }
            """;

    @Test
    void aConstructionIsAReadingOfTheIssueAndAClauseThatEndsIsItsStatus(@TempDir Path into)
            throws Exception {
        assertThat(HARNESS).doesNotContain("__asm__").doesNotContain("extern");
        NativeCompiler.Library library = NativeCompiler.library(Checked.of(List.of(HELD)), into);
        Path source = into.resolve("host.c");
        Files.writeString(source, Running.spelt(HARNESS), StandardCharsets.UTF_8);
        Path executable = into.resolve("host");
        ran(List.of("cc", "-Wall", "-Werror", "-o", executable.toString(), source.toString(),
                "-I", library.header().getParent().toString(), library.library().toString(),
                "-Wl,-rpath," + library.library().getParent()));

        String said = ran(List.of(executable.toString()));

        assertThat(said).isEqualTo("""
                name built: status 0, [ too_short too_short {"actual":{"int":2},"min":{"int":3}}]
                name read: status 0, [ too_short too_short {"actual":{"int":2},"min":{"int":3}}]
                ratio built: status 0, [ invariant_violation invariant_violation {"clause":{"string":"summed"},"module":{"string":"held"},"type":{"string":"Ratio"}}]
                ratio read: status 0, [ invariant_violation invariant_violation {"clause":{"string":"summed"},"module":{"string":"held"},"type":{"string":"Ratio"}}]
                whole built: status 0, a value
                ended built: 1, nothing written 1
                ended read: 1, nothing written 1
                """);
    }

    private static String ran(List<String> command) throws IOException, InterruptedException {
        Process process = new ProcessBuilder(command).redirectErrorStream(true).start();
        String said = new String(process.getInputStream().readAllBytes(), StandardCharsets.UTF_8);
        assertThat(process.waitFor()).as(String.join(" ", command) + "\n" + said).isZero();
        return said;
    }
}
