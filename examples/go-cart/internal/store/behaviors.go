package store

import (
	"database/sql"
	"errors"
	"fmt"

	"github.com/google/uuid"

	"example.com/go-cart/model"
	"example.com/go-cart/model/com/example/cart/domain"
)

// LoadProduct is loadProduct. No row is the model's own ProductNotFound.
type LoadProduct struct{ Tx *sql.Tx }

func (s LoadProduct) Apply(r *model.Run, id domain.ProductId) (domain.ProductOrProductNotFound, error) {
	var row productRow
	err := s.Tx.QueryRow(`SELECT product_id, on_sale, price FROM product WHERE product_id = ?`, id.Value()).
		Scan(&row.id, &row.onSale, &row.price)
	if errors.Is(err, sql.ErrNoRows) {
		none, err := domain.NewProductNotFound(r)
		return none, err
	}
	if err != nil {
		return nil, err
	}
	product, err := domain.ProductDecoder(r).Decode(productForm.Encode(row))
	return product, err
}

// LoadCart is loadCart. It makes sure the user has a cart row, then reads the cart with the total
// quantity of what is in it in one aggregate query: the items themselves are not loaded, and the
// total is what the capacity rule needs.
//
// Making the row is a write before the command has been decided: a Cart has an id, and the row is
// where a new user's cart gets one. A command the model refuses after it (CartFull) keeps nothing,
// since its route answers a Rollback.
type LoadCart struct{ Tx *sql.Tx }

func (s LoadCart) Apply(r *model.Run, userID domain.UserId) (domain.Cart, error) {
	user := userID.Value()
	if _, err := s.Tx.Exec(`INSERT OR IGNORE INTO cart (cart_id, user_id) VALUES (?, ?)`,
		uuid.NewString(), user); err != nil {
		return domain.Cart{}, err
	}
	var row cartRow
	err := s.Tx.QueryRow(`
		SELECT c.cart_id, COALESCE(SUM(ci.quantity), 0)
		FROM cart c LEFT JOIN cart_item ci ON ci.cart_id = c.cart_id
		WHERE c.user_id = ?
		GROUP BY c.cart_id`, user).Scan(&row.id, &row.quantity)
	if err != nil {
		return domain.Cart{}, err
	}
	return domain.CartDecoder(r).Decode(cartForm.Encode(row))
}

// SaveItem is saveItem. It takes the cart and the item out of the PendingItem the model built, adds
// the quantity to the row already there or inserts one, in one statement, and answers what it wrote.
//
// A PendingItem exists only where the capacity holds, so nothing here checks it again.
type SaveItem struct{ Tx *sql.Tx }

func (s SaveItem) Apply(r *model.Run, pending domain.PendingItem) (domain.ItemAdded, error) {
	cartID := pending.Cart().Id().Value()
	item := pending.Item()
	productID, quantity := item.ProductId().Value(), item.Quantity().Value()

	if _, err := s.Tx.Exec(`
		INSERT INTO cart_item (cart_item_id, cart_id, product_id, quantity) VALUES (?, ?, ?, ?)
		ON CONFLICT (cart_id, product_id) DO UPDATE SET quantity = quantity + excluded.quantity`,
		uuid.NewString(), cartID, productID, quantity); err != nil {
		return domain.ItemAdded{}, err
	}
	return domain.NewItemAdded(r, item.ProductId(), item.Quantity())
}

// PriceCart is priceCart. It reads every line of the cart with its product in one query, sees that
// each product is there and on sale, and answers the lines with their prices as a PricedCart. A
// product that is gone or no longer on sale ends it with the model's own case. Where several lines
// cannot be priced, which case answers is the first one read: the model states no order between
// them, and neither does the HTTP contract, so nothing may rely on one.
//
// Deciding that for each line stays here, in the implementation: the model has no traverse, and a
// fold cannot call another injected behavior. The products are joined to the lines rather than
// asked for one by one, so a cart is one query however many lines it has.
type PriceCart struct{ Tx *sql.Tx }

func (s PriceCart) Apply(r *model.Run, userID domain.UserId) (domain.PricedCartOrProductNotFoundOrSaleEnded, error) {
	lines, err := s.lines(userID.Value())
	switch {
	case errors.Is(err, errProductGone):
		gone, err := domain.NewProductNotFound(r)
		return gone, err
	case errors.Is(err, errSaleOver):
		over, err := domain.NewSaleEnded(r)
		return over, err
	case err != nil:
		return nil, err
	}
	priced, err := domain.PricedCartDecoder(r).Decode(pricedCartForm.Encode(lines))
	return priced, err
}

// Why a line of a cart has no price.
var (
	errProductGone = errors.New("a product in the cart is gone")
	errSaleOver    = errors.New("a product in the cart is no longer on sale")
)

// lines are the lines of the user's cart with their prices, or why the first line without one has
// none.
func (s PriceCart) lines(user string) ([]lineRow, error) {
	rows, err := s.Tx.Query(`
		SELECT ci.product_id, ci.quantity, p.on_sale, p.price
		FROM cart_item ci
		JOIN cart c ON c.cart_id = ci.cart_id
		LEFT JOIN product p ON p.product_id = ci.product_id
		WHERE c.user_id = ?
		ORDER BY ci.product_id`, user)
	if err != nil {
		return nil, err
	}
	defer rows.Close()
	var lines []lineRow
	for rows.Next() {
		var line lineRow
		var onSale sql.NullBool
		var price sql.NullInt64
		if err := rows.Scan(&line.productID, &line.quantity, &onSale, &price); err != nil {
			return nil, err
		}
		if !onSale.Valid || !price.Valid {
			return nil, errProductGone
		}
		if !onSale.Bool {
			return nil, errSaleOver
		}
		line.unitPrice = price.Int64
		lines = append(lines, line)
	}
	return lines, rows.Err()
}

// SaveOrder is saveOrder: one row for the order and one for each of its lines. The orderer is laid
// out flat by which case it is, since an individual and a corporation fill different columns.
type SaveOrder struct{ Tx *sql.Tx }

func (s SaveOrder) Apply(r *model.Run, order domain.Order) (domain.OrderPlaced, error) {
	var kind, email string
	var name, companyName, corporateNumber sql.NullString
	switch orderer := order.Orderer().Case().(type) {
	case domain.Individual:
		kind, email = "individual", orderer.Email().Value()
		name = present(orderer.Name().Value())
	case domain.Corporation:
		kind, email = "corporation", orderer.Email().Value()
		companyName = present(orderer.CompanyName().Value())
		corporateNumber = present(orderer.CorporateNumber().Value())
	default:
		// A case the model gains has no columns here until they are written, and is not saved as
		// a row of neither case. go-check-sumtype finds this switch first, when the binding is built.
		return domain.OrderPlaced{}, fmt.Errorf("saveOrder has no row for an orderer that is a %T", orderer)
	}
	orderID, charge := order.Id().Value(), order.Charge()

	if _, err := s.Tx.Exec(`
		INSERT INTO orders (order_id, user_id, subtotal, discount, total, orderer_type, orderer_email,
		                    orderer_name, orderer_company_name, orderer_corporate_number)
		VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?)`,
		orderID, order.UserId().Value(), charge.Subtotal().Value(), charge.Discount().Value(),
		charge.Total().Value(), kind, email, name, companyName, corporateNumber); err != nil {
		return domain.OrderPlaced{}, err
	}
	insert, err := s.Tx.Prepare(`
		INSERT INTO order_line (order_line_id, order_id, product_id, quantity, unit_price)
		VALUES (?, ?, ?, ?, ?)`)
	if err != nil {
		return domain.OrderPlaced{}, err
	}
	defer insert.Close()
	for _, line := range order.Lines() {
		if _, err := insert.Exec(uuid.NewString(), orderID, line.ProductId().Value(),
			line.Quantity().Value(), line.UnitPrice().Value()); err != nil {
			return domain.OrderPlaced{}, err
		}
	}
	return domain.NewOrderPlaced(r, order)
}

func present(text string) sql.NullString { return sql.NullString{String: text, Valid: true} }
