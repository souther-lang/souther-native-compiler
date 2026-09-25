-- A demo user with a cart, and two products. INSERT OR IGNORE so it can run again.
INSERT OR IGNORE INTO cart (cart_id, user_id) VALUES
    ('22222222-2222-2222-2222-222222222222', '11111111-1111-1111-1111-111111111111');

-- On sale, at 1200.
INSERT OR IGNORE INTO product (product_id, name, on_sale, price) VALUES
    ('33333333-3333-3333-3333-333333333333', 'Coffee Beans 500g', 1, 1200);

-- No longer on sale, at 800.
INSERT OR IGNORE INTO product (product_id, name, on_sale, price) VALUES
    ('44444444-4444-4444-4444-444444444444', 'Discontinued Mug', 0, 800);
