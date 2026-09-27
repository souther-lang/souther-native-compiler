//! The wiring: the injected behaviors implemented, and the behaviors the routes call bound to them.
//!
//! It is done for each request, inside the transaction the request runs in: an implementation
//! of an injected behavior borrows that transaction, and the behaviors the routes call borrow the
//! implementations. So the borrow checker, and not a convention, keeps a callback from the library
//! from reaching a transaction that has been committed or rolled back. What binding costs is a box
//! for each implementation.

use std::fmt::Display;
use std::sync::{Arc, Mutex, PoisonError};

use axum::response::Response;
use model::com::example::cart::domain::{
    AddItemToCart, IssueQuote, LoadCartImplementation, LoadProductImplementation, PlaceOrder,
    PriceCartImplementation, SaveItemImplementation, SaveOrderImplementation,
};
use model::{Failure, Library, Run};
use rusqlite::Connection;

use crate::db::{SqlLoadCart, SqlLoadProduct, SqlPriceCart, SqlSaveItem, SqlSaveOrder};
use crate::http::response;

/// The library and the database, shared by every request.
#[derive(Clone)]
pub struct App {
    library: Arc<Library>,
    // One connection, taken by one request at a time. SQLite writes one transaction at a time
    // whatever the application does, and an example has no need of a pool to say so.
    database: Arc<Mutex<Connection>>,
}

/// The three behaviors the routes call, bound for one request.
pub struct Behaviors<'a> {
    pub add_item_to_cart: AddItemToCart<'a>,
    pub place_order: PlaceOrder<'a>,
    pub issue_quote: IssueQuote<'a>,
}

impl App {
    /// The application over `library` and `connection`, which is given the schema and seeded with
    /// the demo user and products.
    ///
    /// # Errors
    ///
    /// Where the schema or the seed cannot be written.
    pub fn new(library: Library, connection: Connection) -> rusqlite::Result<Self> {
        connection.execute_batch(include_str!("../resources/schema.sql"))?;
        connection.execute_batch(include_str!("../resources/data.sql"))?;
        Ok(App {
            library: Arc::new(library),
            database: Arc::new(Mutex::new(connection)),
        })
    }

    /// The response `work` comes to, in one run of the library and one transaction.
    ///
    /// A run is a mark on an arena that belongs to the thread it was opened on, and every value of
    /// the model made in it is good until it ends. So it cannot be held across an `.await`, which
    /// may resume on another thread: `work` is synchronous, and runs on a thread of its own. What
    /// comes back out of it is a `Response`, whose body is text by then; a value of the model does
    /// not compile there.
    pub async fn handle<W>(&self, work: W) -> Response
    where
        W: for<'run> FnOnce(
                &Behaviors<'_>,
                &mut Run<'run>,
                &Connection,
            ) -> Result<Response, Failure>
            + Send
            + 'static,
    {
        let app = self.clone();
        match tokio::task::spawn_blocking(move || app.serve(work)).await {
            Ok(response) => response,
            // A panic in an implementation comes back out of the call that reached it, never
            // through the library, and ends here. The transaction was rolled back as it unwound.
            Err(ended) => internal(ended),
        }
    }

    fn serve<W>(&self, work: W) -> Response
    where
        W: for<'run> FnOnce(
            &Behaviors<'_>,
            &mut Run<'run>,
            &Connection,
        ) -> Result<Response, Failure>,
    {
        // A request that panicked left no transaction open, so what it held is still whole.
        let mut connection = self.database.lock().unwrap_or_else(PoisonError::into_inner);
        let transaction = match connection.transaction() {
            Ok(transaction) => transaction,
            Err(error) => return internal(error),
        };
        let answered = {
            let library = &*self.library;
            let load_product =
                LoadProductImplementation::new(library, SqlLoadProduct(&transaction));
            let load_cart = LoadCartImplementation::new(library, SqlLoadCart(&transaction));
            let save_item = SaveItemImplementation::new(library, SqlSaveItem(&transaction));
            let price_cart = PriceCartImplementation::new(library, SqlPriceCart(&transaction));
            let save_order = SaveOrderImplementation::new(library, SqlSaveOrder(&transaction));
            let behaviors = Behaviors {
                add_item_to_cart: AddItemToCart::bind(
                    library,
                    &load_product,
                    &load_cart,
                    &save_item,
                ),
                place_order: PlaceOrder::bind(library, &price_cart, &save_order),
                issue_quote: IssueQuote::bind(library, &price_cart),
            };
            library.run(|run| work(&behaviors, run, &transaction))
        };
        // Everything that borrowed the transaction has gone out of scope above, so it can be
        // committed. Dropped instead, it rolls back.
        match answered {
            Ok(Ok(response)) => match transaction.commit() {
                Ok(()) => response,
                Err(error) => internal(error),
            },
            Ok(Err(failure)) => internal(failure),
            Err(already_running) => internal(already_running),
        }
    }
}

fn internal(reason: impl Display) -> Response {
    eprintln!("a request ended without an answer: {reason}");
    response::internal()
}
