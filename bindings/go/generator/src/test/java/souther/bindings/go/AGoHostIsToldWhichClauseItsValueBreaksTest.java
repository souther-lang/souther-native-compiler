package souther.bindings.go;

import org.junit.jupiter.api.Test;
import org.junit.jupiter.api.io.TempDir;

import java.nio.file.Path;

import static org.assertj.core.api.Assertions.assertThat;

/**
 * A value a Go host builds with {@code New<Type>} and a clause refuses comes back as the issue the
 * type's {@code Decode<Type>} answers for the same value: the constraint a clause is, with what
 * Raoh's own constraint says of it, or the type and the clause. So which rule the value broke
 * reaches a host whichever way it built the value, and of a type with two clauses it says which.
 */
class AGoHostIsToldWhichClauseItsValueBreaksTest {

    private static final String MEMBERS = """
            module members exposing ( MemberId, Title, Line )

            data MemberId = String
                invariant shaped = String.matches("M[0-9]{5}", value)

            data Title = String
                invariant named = String.length(String.trim(value)) >= 1

            data Line = { quantity: Int, price: Int }
                invariant counted = quantity >= 1
                invariant priced = price >= 0
            """;

    private static final String HOST = """
            package main

            import (
            	"errors"
            	"fmt"
            	"os"
            	"sort"
            	"strings"

            	"example.com/members"
            	model "example.com/members/members"

            	"github.com/raoh-project/raoh-go"
            )

            func said[T any](_ T, err error) string {
            	issues, ok := errors.AsType[*raoh.Issues](err)
            	if !ok {
            		return fmt.Sprintf("not refused: %v", err)
            	}
            	var each []string
            	for _, it := range issues.All() {
            		meta := it.Meta()
            		names := make([]string, 0, len(meta))
            		for name := range meta {
            			names = append(names, name)
            		}
            		sort.Strings(names)
            		var entries []string
            		for _, name := range names {
            			entries = append(entries, fmt.Sprintf("%s=%v", name, meta[name]))
            		}
            		each = append(each, "["+it.Path().String()+" "+it.Code()+" "+strings.Join(entries, " ")+"]")
            	}
            	return strings.Join(each, " ")
            }

            func compared(what, built, read string) {
            	fmt.Printf("%s: %s %t\\n", what, built, built == read)
            }

            func main() {
            	library, err := members.Load(os.Args[1])
            	if err != nil {
            		panic(err)
            	}
            	err = library.Run(func(r *members.Run) error {
            		compared("shaped", said(model.NewMemberId(r, "x1")), said(model.DecodeMemberId(r, []byte(`"x1"`))))
            		compared("named", said(model.NewTitle(r, "  ")), said(model.DecodeTitle(r, []byte(`"  "`))))
            		compared("counted", said(model.NewLine(r, 0, 1)), said(model.DecodeLine(r, []byte(`{"quantity":0,"price":1}`))))
            		compared("priced", said(model.NewLine(r, 1, -1)), said(model.DecodeLine(r, []byte(`{"quantity":1,"price":-1}`))))
            		held, err := model.NewMemberId(r, "M12345")
            		if err != nil {
            			panic(err)
            		}
            		fmt.Printf("held: %s\\n", held.Encode())
            		return nil
            	})
            	if err != nil {
            		panic(err)
            	}
            }
            """;

    @Test
    void aConstructionIsRefusedWithTheIssueItsDecoderAnswers(@TempDir Path into) throws Exception {
        String said = GoHost.ran(into, MEMBERS, "example.com/members", HOST);

        assertThat(said).isEqualTo("""
                shaped: [ invalid_format pattern=M[0-9]{5}] true
                named: [ invariant_violation clause=named module=members type=Title] true
                counted: [ invariant_violation clause=counted module=members type=Line] true
                priced: [ invariant_violation clause=priced module=members type=Line] true
                held: "M12345"
                """);
    }
}
