CREATE TABLE IF NOT EXISTS product (
    product_id TEXT    PRIMARY KEY,
    name       TEXT    NOT NULL,
    on_sale    INTEGER NOT NULL,
    price      INTEGER NOT NULL
);

CREATE TABLE IF NOT EXISTS cart (
    cart_id TEXT PRIMARY KEY,
    user_id TEXT NOT NULL UNIQUE
);

CREATE TABLE IF NOT EXISTS cart_item (
    cart_item_id TEXT    PRIMARY KEY,
    cart_id      TEXT    NOT NULL,
    product_id   TEXT    NOT NULL,
    quantity     INTEGER NOT NULL,
    UNIQUE (cart_id, product_id)
);

CREATE TABLE IF NOT EXISTS orders (
    order_id                 TEXT    PRIMARY KEY,
    user_id                  TEXT    NOT NULL,
    subtotal                 INTEGER NOT NULL,
    discount                 INTEGER NOT NULL,
    total                    INTEGER NOT NULL,
    orderer_type             TEXT    NOT NULL,
    orderer_email            TEXT    NOT NULL,
    orderer_name             TEXT,
    orderer_company_name     TEXT,
    orderer_corporate_number TEXT
);

CREATE TABLE IF NOT EXISTS order_line (
    order_line_id TEXT    PRIMARY KEY,
    order_id      TEXT    NOT NULL,
    product_id    TEXT    NOT NULL,
    quantity      INTEGER NOT NULL,
    unit_price    INTEGER NOT NULL
);
