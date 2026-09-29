// Command go-cart serves the cart of examples/cart-model over HTTP, once bin/build has written the
// library and its binding. It is run from examples/go-cart:
//
//	go run . [-addr localhost:8080] [-library build/native] [-db build/cart.sqlite]
//
// The database is the one CART_DATABASE names where -db is not given.
package main

import (
	"cmp"
	"context"
	"database/sql"
	"errors"
	"flag"
	"log"
	"net/http"
	"os"
	"os/signal"
	"time"

	_ "modernc.org/sqlite"

	"example.com/go-cart/internal/cart"
	"example.com/go-cart/internal/web"
	"example.com/go-cart/model"
)

func main() {
	addr := flag.String("addr", "localhost:8080", "the address to serve on")
	library := flag.String("library", "build/native", "the directory bin/build wrote the library into")
	database := flag.String("db", cmp.Or(os.Getenv("CART_DATABASE"), "build/cart.sqlite"), "the SQLite database")
	flag.Parse()

	if err := run(*addr, *library, *database); err != nil {
		log.Fatal(err)
	}
}

func run(addr, library, database string) error {
	// The library bin/build wrote beside the binding this was compiled against.
	lib, err := model.Load(cart.Library(library))
	if err != nil {
		return err
	}
	db, err := sql.Open("sqlite", database)
	if err != nil {
		return err
	}
	defer db.Close()
	app, err := cart.New(lib, db)
	if err != nil {
		return err
	}

	server := &http.Server{Addr: addr, Handler: web.Router(app), ReadHeaderTimeout: 5 * time.Second}
	ctx, stop := signal.NotifyContext(context.Background(), os.Interrupt)
	defer stop()
	go func() {
		<-ctx.Done()
		shutdown, cancel := context.WithTimeout(context.Background(), 10*time.Second)
		defer cancel()
		_ = server.Shutdown(shutdown)
	}()
	log.Printf("serving the cart on %s", addr)
	if err := server.ListenAndServe(); !errors.Is(err, http.ErrServerClosed) {
		return err
	}
	return nil
}
