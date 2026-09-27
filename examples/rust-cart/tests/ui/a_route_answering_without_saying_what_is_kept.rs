// A route answers a response and leaves it to the application to decide whether what the request
// wrote is kept. That the domain answered is not that its answer is to be kept, so every route says
// which, as an Outcome, and a bare response is not one.
use rust_cart::App;
use rust_cart::http::response;

async fn added(app: App) {
    app.handle(|_, _, _| Ok(response::created(None))).await;
}

fn main() {}
