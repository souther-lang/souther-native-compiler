package souther.bindings.go;

import org.junit.jupiter.api.Test;
import org.junit.jupiter.api.io.TempDir;
import souther.nativecode.IssueMetaContract;

import java.nio.file.Path;
import java.util.StringJoiner;

import static org.assertj.core.api.Assertions.assertThat;

/**
 * A Go host is handed each value of an issue's metadata as a value of the type Raoh holds it as, as
 * the JVM's decoder holds it: what the host reads off its {@code raoh.Issue} is held to what the
 * JVM answers ({@link IssueMetaContract}), and holds a value of every type the library writes. The
 * library writing each value as its type is held elsewhere; this is the other half, the runtime
 * making it a Go value, where a type it did not keep would be lost to the host.
 */
class AGoHostIsHandedEachValueOfAnIssueAsItsTypeTest {

    private static String host() {
        StringJoiner rows = new StringJoiner("\n");
        StringJoiner rendered = new StringJoiner("\n");
        for (IssueMetaContract.Row row : IssueMetaContract.ROWS) {
            rows.add("\t\tfmt.Println(\"%s:\", said(meta.%sDecoder(r).Decode(json(`%s`))))"
                    .formatted(row.label(), row.type(), row.document()));
            rendered.add("\t\tfmt.Println(\"%s:\", render(meta.%sDecoder(r).Decode(json(`%s`))))"
                    .formatted(row.label(), row.type(), row.document()));
        }
        return """
                package main

                import (
                	encoding "encoding/json"
                	"errors"
                	"fmt"
                	"os"
                	"sort"
                	"strings"

                	"example.com/shop"
                	"example.com/shop/meta"

                	"github.com/raoh-project/raoh-go"
                	souther "github.com/souther-lang/souther-native-compiler/bindings/go/runtime"
                )

                func quoted(text string) string {
                	written, err := encoding.Marshal(text)
                	if err != nil {
                		panic(err)
                	}
                	return string(written)
                }

                func written(value any) string {
                	switch it := value.(type) {
                	case int64:
                		return fmt.Sprintf("int:%%d", it)
                	case raoh.Decimal:
                		return "decimal:" + it.String()
                	case string:
                		return "string:" + quoted(it)
                	case bool:
                		return fmt.Sprintf("bool:%%t", it)
                	case souther.Date:
                		return "date:" + it.String()
                	case souther.Time:
                		return "time:" + it.String()
                	case souther.DateTime:
                		return "datetime:" + it.String()
                	case souther.Instant:
                		return "instant:" + it.String()
                	case []any:
                		var items []string
                		for _, item := range it {
                			items = append(items, written(item))
                		}
                		return "list:[" + strings.Join(items, ",") + "]"
                	}
                	panic(fmt.Sprintf("metadata of no type the library writes: %%T", value))
                }

                func said[T any](_ T, err error) string {
                	if err == nil {
                		return "ok"
                	}
                	issues, ok := errors.AsType[*raoh.Issues](err)
                	if !ok {
                		return "failed: " + err.Error()
                	}
                	var each []string
                	for _, it := range issues.All() {
                		key := ""
                		if it.MessageKey() != it.Code() {
                			key = " key=" + it.MessageKey()
                		}
                		meta := it.Meta()
                		names := make([]string, 0, len(meta))
                		for name := range meta {
                			names = append(names, name)
                		}
                		sort.Strings(names)
                		var entries []string
                		for _, name := range names {
                			entries = append(entries, quoted(name)+":"+written(meta[name]))
                		}
                		each = append(each, "["+it.Path().String()+" "+it.Code()+key+" {"+strings.Join(entries, ",")+"}]")
                	}
                	return strings.Join(each, " ")
                }

                func render[T any](_ T, err error) string {
                	if err == nil {
                		return "ok"
                	}
                	issues, ok := errors.AsType[*raoh.Issues](err)
                	if !ok {
                		return "failed: " + err.Error()
                	}
                	var each []string
                	for _, it := range issues.Render(raoh.English) {
                		meta, err := encoding.Marshal(it.Meta)
                		if err != nil {
                			panic(err)
                		}
                		each = append(each, "["+it.Path+" "+it.Code+" "+string(meta)+"]")
                	}
                	return strings.Join(each, " ")
                }

                func main() {
                	library, err := shop.Load(os.Args[1])
                	if err != nil {
                		panic(err)
                	}
                	err = library.Run(func(r *shop.Run) error {
                		json := func(text string) any {
                			v, err := raoh.DecodeJSON([]byte(text), raoh.NewDecoder(func(in any) (any, error) { return in, nil }))
                			if err != nil {
                				panic(err)
                			}
                			return v
                		}
                %s
                		fmt.Println("--")
                %s
                		return nil
                	})
                	if err != nil {
                		panic(err)
                	}
                }
                """.formatted(rows, rendered);
    }

    @Test
    void eachValueIsTheTypeTheJvmHoldsItAs(@TempDir Path into) throws Exception {
        String said = GoHost.ran(into, IssueMetaContract.MODULE, "example.com/shop", host());

        assertThat(said).isEqualTo(IssueMetaContract.answered(IssueMetaContract.DecimalWritten.NUMBER));
        assertThat(IssueMetaContract.typesIn(said)).isEqualTo(IssueMetaContract.metaTypes());
    }
}
