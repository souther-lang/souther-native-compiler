// Command go-cart serves the cart of examples/cart-model on localhost:8080, once bin/build has
// written the library and its binding, over the database build/cart.sqlite unless CART_DATABASE
// names another.
package main

import (
	"cmp"
	"database/sql"
	"log"
	"net/http"
	"os"

	_ "modernc.org/sqlite"

	"example.com/go-cart/internal/cart"
	"example.com/go-cart/internal/web"
	"example.com/go-cart/model"
)

func main() {
	// The library bin/build wrote beside the binding this was compiled against.
	library, err := model.Load(cart.Library())
	if err != nil {
		log.Fatal(err)
	}
	db, err := sql.Open("sqlite", cmp.Or(os.Getenv("CART_DATABASE"), "build/cart.sqlite"))
	if err != nil {
		log.Fatal(err)
	}
	app, err := cart.New(library, db)
	if err != nil {
		log.Fatal(err)
	}
	log.Fatal(http.ListenAndServe("localhost:8080", web.Router(app)))
}
