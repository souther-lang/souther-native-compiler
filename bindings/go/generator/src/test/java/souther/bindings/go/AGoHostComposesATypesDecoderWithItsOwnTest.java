package souther.bindings.go;

import org.junit.jupiter.api.Test;
import org.junit.jupiter.api.io.TempDir;

import java.nio.file.Path;

import static org.assertj.core.api.Assertions.assertThat;

/**
 * A Go host composes a type's decoder with raoh-go's own, as a JVM host composes a type's
 * {@code decoder()}: raoh checks the form of what came in, and the model's decoder reads what it
 * handed on as the type, checking what the type states and telling a sum's cases apart.
 *
 * <p>What the model finds wrong is an issue at the path the decoder was reached at, beside raoh's
 * own, so a host answers both the same way. What raoh hands on is its input model, an object in the
 * order its members came in, which is written back as the JSON it is for the library to read.
 */
class AGoHostComposesATypesDecoderWithItsOwnTest {

    private static final String ORDERING = """
            module ordering exposing ( Email, Individual, Corporation, Orderer, Quantity, Line, Lines,
                                       Free, Voucher, Perks )

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
            data Perks = { perks: List<Free> }
            """;

    private static final String HOST = """
            package main

            import (
            	"errors"
            	"fmt"
            	"math"
            	"os"
            	"strings"

            	"example.com/shop"
            	"example.com/shop/ordering"

            	"github.com/raoh-project/raoh-go"
            )

            func said[T any](value T, err error) string {
            	if err == nil {
            		return fmt.Sprintf("ok %T", value)
            	}
            	issues, ok := errors.AsType[*raoh.Issues](err)
            	if !ok {
            		return "failed: " + err.Error()
            	}
            	var each []string
            	for _, it := range issues.All() {
            		path := it.Path().String()
            		if path == "" {
            			path = "/"
            		}
            		key := ""
            		if it.MessageKey() != it.Code() {
            			key = " key=" + it.MessageKey()
            		}
            		each = append(each, "["+path+" "+it.Code()+key+"]")
            	}
            	return strings.Join(each, " ")
            }

            func field[T any](name string, d raoh.Decoder[any, T]) raoh.Decoder[any, T] {
            	return raoh.Object(raoh.Fields().Field(name, d)).Map(func(v T) T { return v })
            }

            func main() {
            	library, err := shop.Load(os.Args[1])
            	if err != nil {
            		panic(err)
            	}
            	err = library.Run(func(r *shop.Run) error {
            		checkout := field("orderer", ordering.OrdererDecoder(r))
            		json := func(text string) any {
            			// What raoh.DecodeJSON reads, handed on as it is.
            			v, err := raoh.DecodeJSON([]byte(text), raoh.NewDecoder(func(in any) (any, error) { return in, nil }))
            			if err != nil {
            				panic(err)
            			}
            			return v
            		}
            		fmt.Println("individual:", said(checkout.Decode(json(`{"orderer":{"type":"Individual","email":"a@b","name":"Taro"}}`))))
            		orderer, _ := checkout.Decode(json(`{"orderer":{"type":"Individual","email":"a@b","name":"Taro"}}`))
            		individual := orderer.Case().(ordering.Individual)
            		fmt.Println("read:", individual.Email().Value(), individual.Name())
            		fmt.Println("corporation:", said(checkout.Decode(json(`{"orderer":{"type":"Corporation","email":"x@y","companyName":"Acme"}}`))))
            		fmt.Println("short email:", said(checkout.Decode(json(`{"orderer":{"type":"Individual","email":"ab","name":"Taro"}}`))))
            		fmt.Println("no such case:", said(checkout.Decode(json(`{"orderer":{"type":"Robot"}}`))))
            		fmt.Println("missing field:", said(checkout.Decode(json(`{"orderer":{"type":"Corporation","email":"x@y"}}`))))
            		fmt.Println("missing member:", said(checkout.Decode(json(`{}`))))

            		// A field of the host's own beside one of the model's: both are checked, and each
            		// issue is at its own path.
            		both := raoh.Object(raoh.Fields().
            			Field("who", raoh.String().MinLength(2)).
            			Field("individual", ordering.IndividualDecoder(r)),
            		).Map(func(_ string, it ordering.Individual) ordering.Individual { return it })
            		fmt.Println("both:", said(both.Decode(json(`{"who":"x","individual":{"email":"ab","name":"T"}}`))))

            		fmt.Println("quantity:", said(field("n", ordering.QuantityDecoder(r)).Decode(json(`{"n":0}`))))
            		fmt.Println("a float:", said(ordering.QuantityDecoder(r).Decode(json(`2.0`))))
            		fmt.Println("lines:", said(ordering.LinesDecoder(r).Decode(json(`{"lines":[{"sku":"a","quantity":1},{"sku":"b","quantity":0}]}`))))
            		fmt.Println("no lines:", said(ordering.LinesDecoder(r).Decode(json(`{"lines":[]}`))))
            		fmt.Println("not json:", said(ordering.QuantityDecoder(r).Decode(math.NaN())))
            		fmt.Println("a go value:", said(ordering.LinesDecoder(r).Decode(map[string]any{"lines": []any{map[string]any{"sku": "c", "quantity": 2}}})))
            		fmt.Println("a unit:", said(ordering.FreeDecoder(r).Decode(json(`{}`))))
            		fmt.Println("a unit in a product:", said(ordering.VoucherDecoder(r).Decode(json(`{"note":"x","perk":{}}`))))
            		fmt.Println("a list of units:", said(ordering.PerksDecoder(r).Decode(json(`{"perks":[{},{}]}`))))
            		fmt.Println("case alone:", said(ordering.CorporationDecoder(r).Decode(json(`{"email":"x@y","companyName":"Acme"}`))))
            		return nil
            	})
            	if err != nil {
            		panic(err)
            	}
            }
            """;

    @Test
    void aTypesDecoderComposesWithRaohsAndReportsAtItsPath(@TempDir Path into) throws Exception {
        String said = GoHost.ran(into, ORDERING, "example.com/shop", HOST);

        assertThat(said).isEqualTo("""
                individual: ok ordering.Orderer
                read: a@b Taro
                corporation: ok ordering.Orderer
                short email: [/orderer/email too_short]
                no such case: [/orderer/type not_allowed]
                missing field: [/orderer/companyName missing_field]
                missing member: [/orderer required]
                both: [/who too_short] [/individual/email too_short]
                quantity: [/n out_of_range key=out_of_range.positive]
                a float: [/ type_mismatch]
                lines: [/lines/1/quantity out_of_range key=out_of_range.positive]
                no lines: ok ordering.Lines
                not json: [/ type_mismatch]
                a go value: ok ordering.Lines
                a unit: ok ordering.Free
                a unit in a product: ok ordering.Voucher
                a list of units: ok ordering.Perks
                case alone: ok ordering.Corporation
                """);
    }
}
