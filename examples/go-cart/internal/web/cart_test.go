// The HTTP contract every host of the cart example keeps, over the native library and SQLite: 201
// where an item is added or an order placed, 422 for a business case the model answers, 400 for an
// input that does not decode. The capacity is the 10000 PendingItem states in cart.sou. An order
// and a quotation come back as the model writes them.
//
// Each test starts from a database of its own, seeded as the application seeds it.
package web_test

import (
	"database/sql"
	"encoding/json"
	"net/http"
	"net/http/httptest"
	"path/filepath"
	"reflect"
	"slices"
	"strings"
	"sync"
	"testing"
	"time"

	_ "modernc.org/sqlite"

	"example.com/go-cart/internal/cart"
	"example.com/go-cart/internal/web"
	"example.com/go-cart/model"
)

const (
	user    = "11111111-1111-1111-1111-111111111111"
	onSale  = "33333333-3333-3333-3333-333333333333"
	offSale = "44444444-4444-4444-4444-444444444444"
)

type object = map[string]any

var library = sync.OnceValue(func() *model.Library {
	// The library bin/build wrote beside the binding this test was compiled against.
	library, err := model.Load(cart.Library("../../build/native"))
	if err != nil {
		panic("run bin/build first: " + err.Error())
	}
	return library
})

type client struct {
	t       *testing.T
	handler http.Handler
}

type answer struct {
	status int
	body   any
}

func newClient(t *testing.T) client {
	return over(t, ":memory:")
}

func over(t *testing.T, database string) client {
	t.Helper()
	db, err := sql.Open("sqlite", database)
	if err != nil {
		t.Fatal(err)
	}
	t.Cleanup(func() { db.Close() })
	app, err := cart.New(library(), db)
	if err != nil {
		t.Fatal(err)
	}
	return client{t, web.Router(app)}
}

func (c client) addItem(userID, productID string, quantity int64) answer {
	return c.post("/carts/items", object{"userId": userID, "productId": productID, "quantity": quantity})
}

func (c client) checkout(path, userID string, orderer object) answer {
	return c.post(path, object{"userId": userID, "orderer": orderer})
}

func (c client) post(path string, body any) answer {
	text, ok := body.(string)
	if !ok {
		written, err := json.Marshal(body)
		if err != nil {
			c.t.Fatal(err)
		}
		text = string(written)
	}
	return c.send(httptest.NewRequest(http.MethodPost, path, strings.NewReader(text)))
}

func (c client) get(uri string) answer {
	return c.send(httptest.NewRequest(http.MethodGet, uri, nil))
}

func (c client) send(req *http.Request) answer {
	c.t.Helper()
	recorder := httptest.NewRecorder()
	c.handler.ServeHTTP(recorder, req)
	var body any
	if recorder.Body.Len() > 0 {
		if err := json.Unmarshal(recorder.Body.Bytes(), &body); err != nil {
			c.t.Fatalf("%s: %v", recorder.Body, err)
		}
	}
	return answer{recorder.Code, body}
}

func individual() object {
	return object{"type": "Individual", "email": "Taro@Example.com ", "name": "山田太郎"}
}

func corporation() object {
	return object{"type": "Corporation", "email": "info@acme.co.jp", "companyName": "Acme株式会社",
		"corporateNumber": "1234567890123"}
}

func with(o object, key string, value any) object {
	o[key] = value
	return o
}

func (a answer) field(path ...string) any {
	at := a.body
	for _, name := range path {
		at = at.(object)[name]
	}
	return at
}

func (a answer) paths() []string {
	var paths []string
	for _, issue := range a.field("issues").([]any) {
		paths = append(paths, issue.(object)["path"].(string))
	}
	slices.Sort(paths)
	return paths
}

func expect(t *testing.T, a answer, status int, body any) {
	t.Helper()
	if a.status != status {
		t.Fatalf("status %d, not %d: %v", a.status, status, a.body)
	}
	if body != nil && !reflect.DeepEqual(a.body, normalized(t, body)) {
		t.Fatalf("answered %v, not %v", a.body, body)
	}
}

func expectIssues(t *testing.T, a answer, paths ...string) {
	t.Helper()
	expect(t, a, http.StatusBadRequest, nil)
	if got := a.paths(); !slices.Equal(got, paths) {
		t.Fatalf("issues at %v, not %v: %v", got, paths, a.body)
	}
}

// normalized is v as JSON reads it back, numbers as float64.
func normalized(t *testing.T, v any) any {
	text, err := json.Marshal(v)
	if err != nil {
		t.Fatal(err)
	}
	var back any
	if err := json.Unmarshal(text, &back); err != nil {
		t.Fatal(err)
	}
	return back
}

func TestAnItemIsAddedWhereTheModelSaysItIs(t *testing.T) {
	for _, each := range []struct {
		name      string
		productID string
		quantity  int64
		status    int
		body      any
	}{
		{"an item on sale", onSale, 8, http.StatusCreated, nil},
		{"a quantity over the capacity", onSale, 10001, http.StatusUnprocessableEntity, object{"error": "cart_full"}},
		{"an item no longer on sale", offSale, 1, http.StatusUnprocessableEntity, object{"error": "sale_ended"}},
		{"a product nobody sells", "55555555-5555-5555-5555-555555555555", 1, http.StatusUnprocessableEntity,
			object{"error": "product_not_found"}},
	} {
		t.Run(each.name, func(t *testing.T) {
			expect(t, newClient(t).addItem(user, each.productID, each.quantity), each.status, each.body)
		})
	}
}

func TestABodyTheRouteCannotReadIsTheClients(t *testing.T) {
	for _, each := range []struct {
		name   string
		body   any
		status int
		paths  []string
	}{
		{"a quantity of none", object{"userId": user, "productId": onSale, "quantity": 0},
			http.StatusBadRequest, []string{"/quantity"}},
		{"no quantity", object{"userId": user, "productId": onSale}, http.StatusBadRequest, []string{"/quantity"}},
		{"not JSON", "{", http.StatusBadRequest, []string{""}},
		{"longer than a body may be", strings.Repeat(" ", 1<<20+1), http.StatusRequestEntityTooLarge, nil},
	} {
		t.Run(each.name, func(t *testing.T) {
			a := newClient(t).post("/carts/items", each.body)

			expect(t, a, each.status, nil)
			if each.paths != nil {
				expectIssues(t, a, each.paths...)
			}
		})
	}
}

func TestAnIDIsAUUIDAsThisAPIWritesOne(t *testing.T) {
	// Upper case is written in lower case; a UUID in another notation is not how an id is written.
	for _, each := range []struct {
		name, userID string
		status       int
	}{
		{"in upper case", strings.ToUpper(user), http.StatusCreated},
		{"braced", "{" + user + "}", http.StatusBadRequest},
		{"without hyphens", strings.ReplaceAll(user, "-", ""), http.StatusBadRequest},
	} {
		t.Run(each.name, func(t *testing.T) {
			expect(t, newClient(t).addItem(each.userID, onSale, 1), each.status, nil)
		})
	}
}

func TestAnOrdererThatDoesNotDecodeIs400WithEveryIssueInIt(t *testing.T) {
	for _, each := range []struct {
		name    string
		userID  string
		orderer object
		paths   []string
	}{
		{"a corporate number other than thirteen digits", user,
			with(corporation(), "corporateNumber", "12345"), []string{"/orderer/corporateNumber"}},
		// That a name is not blank is PersonName's rule, and the model's decoder reports it.
		{"a name of nothing but spaces", user, with(individual(), "name", "   "), []string{"/orderer/name"}},
		{"an orderer of no known type", user, with(individual(), "type", "Robot"), []string{"/orderer/type"}},
		// The email is not shaped like one, which the boundary finds; the corporation has no company
		// name and no corporate number, which only the model can say.
		{"a member the boundary refuses keeps the model from reading nothing else", user,
			object{"type": "Corporation", "email": "not-an-email"},
			[]string{"/orderer/companyName", "/orderer/corporateNumber", "/orderer/email"}},
		// A name that is no text is refused by the boundary, which trims it, and by the model, which
		// reads a PersonName. The boundary's issue says what form it was not in, and is the one kept.
		{"a member both refuse is answered once, by the boundary", user, with(individual(), "name", 5),
			[]string{"/orderer/name"}},
		// raoh finds that the user is no UUID. The model finds that a corporation has a company name
		// and a corporate number, which raoh, reading the fields that are there, has no way to know.
		{"every issue of a request at once, whichever step found it", "not-a-uuid",
			object{"type": "Corporation", "email": "info@acme.co.jp"},
			[]string{"/orderer/companyName", "/orderer/corporateNumber", "/userId"}},
	} {
		t.Run(each.name, func(t *testing.T) {
			expectIssues(t, newClient(t).checkout("/carts/checkout", each.userID, each.orderer), each.paths...)
		})
	}
}

func TestATotalExactlyAtTheCapacityIsAdded(t *testing.T) {
	c := newClient(t)

	expect(t, c.addItem(user, onSale, 9998), http.StatusCreated, nil)
	expect(t, c.addItem(user, onSale, 2), http.StatusCreated, nil)
	expect(t, c.addItem(user, onSale, 1), http.StatusUnprocessableEntity, nil)
}

func TestAnIDThatIsNotAUUIDIs400WithRaohsIssue(t *testing.T) {
	a := newClient(t).addItem("not-a-uuid", onSale, 1)

	expectIssues(t, a, "/userId")
	issue := a.field("issues").([]any)[0].(object)
	if issue["code"] != "invalid_format" || !reflect.DeepEqual(issue["meta"], object{}) {
		t.Fatalf("the issue is %v", issue)
	}
	if _, ok := a.field("errors").(object); !ok {
		t.Fatalf("no errors by path: %v", a.body)
	}
}

func TestAnAddedItemIsListed(t *testing.T) {
	c := newClient(t)
	c.addItem(user, onSale, 3)

	expect(t, c.get("/carts/items?userId="+user), http.StatusOK, object{
		"total": 1, "page": 0, "size": 20,
		"items": []any{object{"productId": onSale, "quantity": 3}},
	})
}

func TestAListingIsPaged(t *testing.T) {
	c := newClient(t)
	c.addItem(user, onSale, 3)

	expect(t, c.get("/carts/items?userId="+user+"&page=1&size=0"), http.StatusOK, object{
		"total": 1, "page": 1, "size": 1, "items": []any{},
	})
}

func TestAnIndividualChecksOutWithTheDiscount(t *testing.T) {
	c := newClient(t)
	buyer := "11111111-1111-1111-1111-111111111112"
	// 1200 x 8 = 9600, which is at least 5000: 10% off is 960, and the total 8640.
	c.addItem(buyer, onSale, 8)

	a := c.checkout("/carts/checkout", buyer, individual())

	expect(t, a, http.StatusCreated, nil)
	for _, each := range []struct {
		field    string
		expected any
	}{
		{"userId", buyer},
		{"orderer", object{"type": "Individual", "email": "taro@example.com", "name": "山田太郎"}},
		{"charge", object{"subtotal": 9600, "discount": 960, "total": 8640}},
		{"lines", []any{object{"productId": onSale, "quantity": 8, "unitPrice": 1200}}},
	} {
		if got := a.field(each.field); !reflect.DeepEqual(got, normalized(t, each.expected)) {
			t.Errorf("%s is %v, not %v", each.field, got, each.expected)
		}
	}
}

func TestACorporationChecksOutWithoutTheDiscountUnder5000(t *testing.T) {
	c := newClient(t)
	buyer := "11111111-1111-1111-1111-111111111114"
	c.addItem(buyer, onSale, 3)

	a := c.checkout("/carts/checkout", buyer, corporation())

	expect(t, a, http.StatusCreated, nil)
	if got := a.field("orderer"); !reflect.DeepEqual(got, normalized(t, corporation())) {
		t.Errorf("the orderer is %v", got)
	}
	if got := a.field("charge"); !reflect.DeepEqual(got,
		normalized(t, object{"subtotal": 3600, "discount": 0, "total": 3600})) {
		t.Errorf("the charge is %v", got)
	}
}

func TestANameIsKeptWithoutTheSpacesAroundIt(t *testing.T) {
	// Trimming is how the boundary writes a name, not a rule the model states, so the model is
	// handed the name without them and its bound is on what it keeps.
	c := newClient(t)
	buyer := "11111111-1111-1111-1111-111111111119"
	c.addItem(buyer, onSale, 1)
	longest := strings.Repeat("名", 100)

	a := c.checkout("/carts/checkout", buyer, with(individual(), "name", "  "+longest+"  "))

	expect(t, a, http.StatusCreated, nil)
	if got := a.field("orderer", "name"); got != longest {
		t.Fatalf("the name is %q", got)
	}
}

func TestACompanyNameIsKeptWithoutTheSpacesAroundIt(t *testing.T) {
	c := newClient(t)
	buyer := "11111111-1111-1111-1111-11111111111a"
	c.addItem(buyer, onSale, 1)

	a := c.checkout("/carts/checkout", buyer, with(corporation(), "companyName", "  Acme株式会社 "))

	expect(t, a, http.StatusCreated, nil)
	if got := a.field("orderer", "companyName"); got != "Acme株式会社" {
		t.Fatalf("the company name is %q", got)
	}
}

func TestARefusedCommandKeepsNothingItWroteOnTheWay(t *testing.T) {
	// loadCart makes a new user's cart row before the capacity is decided. A command the model
	// refuses keeps nothing, and one it answers keeps what it wrote.
	database := filepath.Join(t.TempDir(), "cart.sqlite")
	c := over(t, database)
	cartsOf := func(userID string) (n int) {
		db, err := sql.Open("sqlite", database)
		if err != nil {
			t.Fatal(err)
		}
		defer db.Close()
		if err := db.QueryRow(`SELECT COUNT(*) FROM cart WHERE user_id = ?`, userID).Scan(&n); err != nil {
			t.Fatal(err)
		}
		return n
	}
	refused, answered := "11111111-1111-1111-1111-11111111111b", "11111111-1111-1111-1111-11111111111c"

	expect(t, c.addItem(refused, onSale, 10001), http.StatusUnprocessableEntity, nil)
	expect(t, c.addItem(answered, onSale, 1), http.StatusCreated, nil)

	if got := [2]int{cartsOf(refused), cartsOf(answered)}; got != [2]int{0, 1} {
		t.Fatalf("carts %v, not [0 1]", got)
	}
}

func TestAFieldTheOrderersCaseHasIsMissingIs400(t *testing.T) {
	// Which fields an individual has is the model's to say, and its decoder says it.
	orderer := individual()
	delete(orderer, "name")

	a := newClient(t).checkout("/carts/checkout", user, orderer)

	expectIssues(t, a, "/orderer/name")
	if code := a.field("issues").([]any)[0].(object)["code"]; code != "required" {
		t.Fatalf("the code is %v", code)
	}
}

func TestAnEmptyCartDoesNotCheckOut(t *testing.T) {
	expect(t, newClient(t).checkout("/carts/checkout", "11111111-1111-1111-1111-111111111113", individual()),
		http.StatusUnprocessableEntity, object{"error": "empty_cart"})
}

func TestACorporationIsQuoted(t *testing.T) {
	c := newClient(t)
	buyer := "11111111-1111-1111-1111-111111111116"
	c.addItem(buyer, onSale, 8)

	// The quotation holds for thirty days from the day it is issued, which is the model's rule; the
	// day is the host's to say, and it says today, whichever side of midnight the request fell on.
	before := time.Now()
	a := c.checkout("/carts/quote", buyer, corporation())
	after := time.Now()

	expect(t, a, http.StatusOK, nil)
	if id := a.field("id").(string); len(id) != 36 {
		t.Errorf("the id is %q", id)
	}
	if kind := a.field("orderer", "type"); kind != "Corporation" {
		t.Errorf("the orderer is a %v", kind)
	}
	if got := a.field("charge"); !reflect.DeepEqual(got,
		normalized(t, object{"subtotal": 9600, "discount": 960, "total": 8640})) {
		t.Errorf("the charge is %v", got)
	}
	validUntil := a.field("validUntil").(string)
	if !slices.Contains([]string{before.AddDate(0, 0, 30).Format(time.DateOnly),
		after.AddDate(0, 0, 30).Format(time.DateOnly)}, validUntil) {
		t.Errorf("valid until %s, not thirty days from today", validUntil)
	}
}

func TestAnIndividualIsNotQuoted(t *testing.T) {
	c := newClient(t)
	buyer := "11111111-1111-1111-1111-111111111117"
	c.addItem(buyer, onSale, 2)

	expect(t, c.checkout("/carts/quote", buyer, individual()), http.StatusUnprocessableEntity,
		object{"error": "quote_for_corporations_only"})
}

func TestAnEmptyCartIsNotQuoted(t *testing.T) {
	expect(t, newClient(t).checkout("/carts/quote", "11111111-1111-1111-1111-111111111118", corporation()),
		http.StatusUnprocessableEntity, object{"error": "empty_cart"})
}

func TestAPageFurtherThanAnyIsEmpty(t *testing.T) {
	c := newClient(t)
	c.addItem(user, onSale, 3)

	expect(t, c.get("/carts/items?userId="+user+"&page=9223372036854775807&size=20"), http.StatusOK, object{
		"total": 1, "page": 9223372036854775807, "size": 20, "items": []any{},
	})
}

// withdrawn is a client over a database of its own in which the products in the buyer's cart are
// changed by change once they are in it. Beside the seeded product on sale, the cart holds second.
func withdrawn(t *testing.T, buyer string, change string) client {
	t.Helper()
	database := filepath.Join(t.TempDir(), "cart.sqlite")
	c := over(t, database)
	db, err := sql.Open("sqlite", database)
	if err != nil {
		t.Fatal(err)
	}
	defer db.Close()
	if _, err := db.Exec(`INSERT INTO product (product_id, name, on_sale, price) VALUES (?, 'Filter Papers', 1, 300)`,
		second); err != nil {
		t.Fatal(err)
	}
	expect(t, c.addItem(buyer, onSale, 1), http.StatusCreated, nil)
	expect(t, c.addItem(buyer, second, 1), http.StatusCreated, nil)
	if _, err := db.Exec(change); err != nil {
		t.Fatal(err)
	}
	return c
}

// second is a product on sale that withdrawn adds beside the seeded one.
const second = "33333333-3333-3333-3333-333333333334"

func TestAProductNoLongerOnSaleByCheckoutIs422(t *testing.T) {
	buyer := "11111111-1111-1111-1111-11111111111d"
	c := withdrawn(t, buyer, `UPDATE product SET on_sale = 0 WHERE product_id = '`+second+`'`)

	expect(t, c.checkout("/carts/checkout", buyer, individual()), http.StatusUnprocessableEntity,
		object{"error": "sale_ended"})
}

func TestAProductGoneByCheckoutIs422(t *testing.T) {
	buyer := "11111111-1111-1111-1111-11111111111e"
	c := withdrawn(t, buyer, `DELETE FROM product WHERE product_id = '`+second+`'`)

	expect(t, c.checkout("/carts/checkout", buyer, individual()), http.StatusUnprocessableEntity,
		object{"error": "product_not_found"})
}
