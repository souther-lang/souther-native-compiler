<?php

declare(strict_types=1);

namespace App\Tests;

use App\CartApplication;
use App\Http\Request;
use App\Http\Response;
use Model\Binding;
use PDO;
use PHPUnit\Framework\Attributes\Test;
use PHPUnit\Framework\TestCase;

/**
 * The HTTP contract of the Java example, over the native library and SQLite: 201 where an item is
 * added or an order placed, 422 for a business case the model answers, 400 for an input that does
 * not decode. The capacity is the 10000 `PendingItem` states in cart.sou. An order and a quotation
 * come back as the model writes them.
 *
 * Each test starts from a database of its own, seeded as the application seeds it.
 */
final class CartIntegrationTest extends TestCase
{
    private const USER = '11111111-1111-1111-1111-111111111111';
    private const ON_SALE = '33333333-3333-3333-3333-333333333333';
    private const OFF_SALE = '44444444-4444-4444-4444-444444444444';
    /** A product on sale that `withdrawn` adds beside the seeded one. */
    private const SECOND = '33333333-3333-3333-3333-333333333334';

    private CartApplication $app;

    private PDO $pdo;

    protected function setUp(): void
    {
        $this->pdo = new PDO('sqlite::memory:');
        $this->app = CartApplication::create(Binding::load(CartApplication::library()), $this->pdo);
    }

    #[Test]
    public function anItemOnSaleIsAdded(): void
    {
        self::assertSame(201, $this->addItem(self::USER, self::ON_SALE, 8)->status);
    }

    #[Test]
    public function aQuantityOverTheCapacityIs422(): void
    {
        $response = $this->addItem(self::USER, self::ON_SALE, 10001);

        self::assertSame(422, $response->status);
        self::assertSame(['error' => 'cart_full'], self::body($response));
    }

    #[Test]
    public function aTotalExactlyAtTheCapacityIsAdded(): void
    {
        self::assertSame(201, $this->addItem(self::USER, self::ON_SALE, 9998)->status);
        self::assertSame(201, $this->addItem(self::USER, self::ON_SALE, 2)->status);
        self::assertSame(422, $this->addItem(self::USER, self::ON_SALE, 1)->status);
    }

    #[Test]
    public function anItemNoLongerOnSaleIs422(): void
    {
        $response = $this->addItem(self::USER, self::OFF_SALE, 1);

        self::assertSame(422, $response->status);
        self::assertSame(['error' => 'sale_ended'], self::body($response));
    }

    #[Test]
    public function aProductNobodySellsIs422(): void
    {
        $response = $this->addItem(self::USER, '55555555-5555-5555-5555-555555555555', 1);

        self::assertSame(422, $response->status);
        self::assertSame(['error' => 'product_not_found'], self::body($response));
    }

    #[Test]
    public function anIdThatIsNotAUuidIs400WithRaohsIssue(): void
    {
        $response = $this->addItem('not-a-uuid', self::ON_SALE, 1);

        self::assertSame(400, $response->status);
        self::assertSame('/userId', self::body($response)['issues'][0]['path']);
        self::assertSame('invalid_format', self::body($response)['issues'][0]['code']);
    }

    #[Test]
    public function anIssuesMapsAreWrittenAsObjectsEvenWhenEmpty(): void
    {
        $response = $this->addItem('not-a-uuid', self::ON_SALE, 1);
        $read = json_decode((string) $response->body, false, flags: JSON_THROW_ON_ERROR);

        self::assertEquals(new \stdClass(), $read->issues[0]->meta, (string) $response->body);
        self::assertIsObject($read->errors);
    }

    #[Test]
    public function anIdIsAUuidAsThisApiWritesOne(): void
    {
        // Upper case is written in lower case; a UUID in another notation is not how an id is written.
        $upper = $this->addItem(strtoupper(self::USER), self::ON_SALE, 1);
        $braced = $this->addItem('{' . self::USER . '}', self::ON_SALE, 1);
        $bare = $this->addItem(str_replace('-', '', self::USER), self::ON_SALE, 1);

        self::assertSame(201, $upper->status);
        self::assertSame([400, ['/userId']], [$braced->status, array_column(self::body($braced)['issues'], 'path')]);
        self::assertSame([400, ['/userId']], [$bare->status, array_column(self::body($bare)['issues'], 'path')]);
    }

    #[Test]
    public function aQuantityOfNoneIs400(): void
    {
        $response = $this->addItem(self::USER, self::ON_SALE, 0);

        self::assertSame(400, $response->status);
        self::assertSame('/quantity', self::body($response)['issues'][0]['path']);
    }

    #[Test]
    public function aBodyThatIsNotJsonIs400(): void
    {
        self::assertSame(400, $this->app->handle(new Request('POST', '/carts/items', [], '{'))->status);
    }

    #[Test]
    public function anAddedItemIsListed(): void
    {
        $this->addItem(self::USER, self::ON_SALE, 3);

        $response = $this->app->handle(new Request('GET', '/carts/items', ['userId' => self::USER]));

        self::assertSame(200, $response->status);
        self::assertSame(
            ['total' => 1, 'page' => 0, 'size' => 20, 'items' => [['productId' => self::ON_SALE, 'quantity' => 3]]],
            self::body($response));
    }

    #[Test]
    public function anIndividualChecksOutWithTheDiscount(): void
    {
        $user = '11111111-1111-1111-1111-111111111112';
        // 1200 x 8 = 9600, which is at least 5000: 10% off is 960, and the total 8640.
        $this->addItem($user, self::ON_SALE, 8);

        $response = $this->checkout('/carts/checkout', $user, self::individual());
        $order = self::body($response);

        self::assertSame(201, $response->status);
        self::assertSame($user, $order['userId']);
        self::assertSame(['type' => 'Individual', 'email' => 'taro@example.com', 'name' => '山田太郎'],
            $order['orderer']);
        self::assertSame(['subtotal' => 9600, 'discount' => 960, 'total' => 8640], $order['charge']);
        self::assertSame([['productId' => self::ON_SALE, 'quantity' => 8, 'unitPrice' => 1200]], $order['lines']);
    }

    #[Test]
    public function aCorporationChecksOutWithoutTheDiscountUnder5000(): void
    {
        $user = '11111111-1111-1111-1111-111111111114';
        $this->addItem($user, self::ON_SALE, 3);

        $response = $this->checkout('/carts/checkout', $user, self::corporation());
        $order = self::body($response);

        self::assertSame(201, $response->status);
        self::assertSame(['type' => 'Corporation', 'email' => 'info@acme.co.jp', 'companyName' => 'Acme株式会社',
            'corporateNumber' => '1234567890123'], $order['orderer']);
        self::assertSame(['subtotal' => 3600, 'discount' => 0, 'total' => 3600], $order['charge']);
    }

    #[Test]
    public function aCorporateNumberOtherThanThirteenDigitsIs400(): void
    {
        $user = '11111111-1111-1111-1111-111111111115';
        $this->addItem($user, self::ON_SALE, 1);

        $response = $this->checkout('/carts/checkout', $user, ['corporateNumber' => '12345'] + self::corporation());

        self::assertSame(400, $response->status);
        self::assertSame('/orderer/corporateNumber', self::body($response)['issues'][0]['path']);
    }

    #[Test]
    public function aNameOfNothingButSpacesIs400(): void
    {
        // That a name is not blank is PersonName's rule, and the model's decoder reports it.
        $response = $this->checkout('/carts/checkout', self::USER, ['name' => '   '] + self::individual());

        self::assertSame(400, $response->status);
        self::assertSame('/orderer/name', self::body($response)['issues'][0]['path']);
    }

    #[Test]
    public function aNameIsKeptWithoutTheSpacesAroundIt(): void
    {
        // Trimming is how the boundary writes a name, not a rule the model states, so the model is
        // handed the name without them and its bound is on what it keeps.
        $user = '11111111-1111-1111-1111-111111111119';
        $this->addItem($user, self::ON_SALE, 1);
        $longest = str_repeat('名', 100);

        $response = $this->checkout('/carts/checkout', $user, ['name' => "  {$longest}  "] + self::individual());

        self::assertSame(201, $response->status, (string) $response->body);
        self::assertSame($longest, self::body($response)['orderer']['name']);
    }

    #[Test]
    public function aCompanyNameIsKeptWithoutTheSpacesAroundIt(): void
    {
        $user = '11111111-1111-1111-1111-11111111111a';
        $this->addItem($user, self::ON_SALE, 1);

        $response = $this->checkout('/carts/checkout', $user, ['companyName' => '  Acme株式会社 '] + self::corporation());

        self::assertSame(201, $response->status, (string) $response->body);
        self::assertSame('Acme株式会社', self::body($response)['orderer']['companyName']);
    }

    #[Test]
    public function aMemberTheBoundaryRefusesDoesNotKeepTheModelFromReadingTheRest(): void
    {
        // The email is not shaped like one, which the boundary finds; the corporation has no
        // company name and no corporate number, which only the model can say.
        $response = $this->checkout('/carts/checkout', self::USER,
            ['type' => 'Corporation', 'email' => 'not-an-email']);

        self::assertSame(400, $response->status);
        $paths = array_column(self::body($response)['issues'], 'path');
        sort($paths);
        self::assertSame(['/orderer/companyName', '/orderer/corporateNumber', '/orderer/email'], $paths,
            (string) $response->body);
    }

    #[Test]
    public function aMemberBothRefuseIsAnsweredOnceByTheBoundary(): void
    {
        // A name that is no text is refused by the boundary, which trims it, and by the model, which
        // reads a PersonName. The boundary's issue says what form it was not in, and is the one kept.
        $response = $this->checkout('/carts/checkout', self::USER, ['name' => 5] + self::individual());

        self::assertSame(400, $response->status);
        self::assertSame(['/orderer/name'], array_column(self::body($response)['issues'], 'path'),
            (string) $response->body);
    }

    #[Test]
    public function aRefusedCommandKeepsNothingItWroteOnTheWay(): void
    {
        // loadCart makes a new user's cart row before the capacity is decided. A command the model
        // refuses keeps nothing, and one it answers keeps what it wrote.
        $refused = '11111111-1111-1111-1111-11111111111b';
        $answered = '11111111-1111-1111-1111-11111111111c';

        self::assertSame(422, $this->addItem($refused, self::ON_SALE, 10001)->status);
        self::assertSame(201, $this->addItem($answered, self::ON_SALE, 1)->status);
        self::assertSame([0, 1], [$this->cartsOf($refused), $this->cartsOf($answered)]);
    }

    #[Test]
    public function anOrdererOfNoKnownTypeIs400(): void
    {
        $response = $this->checkout('/carts/checkout', self::USER, ['type' => 'Robot'] + self::individual());

        self::assertSame(400, $response->status);
        self::assertSame('/orderer/type', self::body($response)['issues'][0]['path']);
    }

    #[Test]
    public function aFieldTheOrderersCaseHasIsMissingIs400(): void
    {
        // Which fields an individual has is the model's to say, and its decoder says it.
        $individual = self::individual();
        unset($individual['name']);

        $response = $this->checkout('/carts/checkout', self::USER, $individual);

        self::assertSame(400, $response->status);
        self::assertSame(['path' => '/orderer/name', 'code' => 'missing_field'],
            array_intersect_key(self::body($response)['issues'][0], ['path' => 0, 'code' => 0]));
    }

    #[Test]
    public function everyIssueOfARequestIsAnsweredAtOnceWhicheverStepFoundIt(): void
    {
        // raoh-php finds that the user is no UUID. The model finds that a corporation has a company
        // name and a corporate number, which raoh-php, reading only the email, has no way to know.
        $response = $this->checkout('/carts/checkout', 'not-a-uuid',
            ['type' => 'Corporation', 'email' => 'info@acme.co.jp']);

        self::assertSame(400, $response->status);
        $paths = array_column(self::body($response)['issues'], 'path');
        sort($paths);
        self::assertSame(['/orderer/companyName', '/orderer/corporateNumber', '/userId'], $paths,
            (string) $response->body);
    }

    #[Test]
    public function anEmptyCartDoesNotCheckOut(): void
    {
        $response = $this->checkout('/carts/checkout', '11111111-1111-1111-1111-111111111113', self::individual());

        self::assertSame(422, $response->status);
        self::assertSame(['error' => 'empty_cart'], self::body($response));
    }

    #[Test]
    public function aCorporationIsQuoted(): void
    {
        $user = '11111111-1111-1111-1111-111111111116';
        $this->addItem($user, self::ON_SALE, 8);

        $response = $this->checkout('/carts/quote', $user, self::corporation());
        $quote = self::body($response);

        self::assertSame(200, $response->status);
        self::assertMatchesRegularExpression('/^[0-9a-f-]{36}$/', $quote['id']);
        self::assertSame('Corporation', $quote['orderer']['type']);
        self::assertSame(['subtotal' => 9600, 'discount' => 960, 'total' => 8640], $quote['charge']);
        self::assertMatchesRegularExpression('/^\d{4}-\d{2}-\d{2}$/', $quote['validUntil']);
    }

    #[Test]
    public function anIndividualIsNotQuoted(): void
    {
        $user = '11111111-1111-1111-1111-111111111117';
        $this->addItem($user, self::ON_SALE, 2);

        self::assertSame(422, $this->checkout('/carts/quote', $user, self::individual())->status);
    }

    #[Test]
    public function anEmptyCartIsNotQuoted(): void
    {
        $response = $this->checkout('/carts/quote', '11111111-1111-1111-1111-111111111118', self::corporation());

        self::assertSame(422, $response->status);
        self::assertSame(['error' => 'empty_cart'], self::body($response));
    }

    #[Test]
    public function aProductNoLongerOnSaleByCheckoutIs422(): void
    {
        $user = '11111111-1111-1111-1111-11111111111d';
        $this->withdrawn($user, "UPDATE product SET on_sale = 0 WHERE product_id = '" . self::SECOND . "'");

        $response = $this->checkout('/carts/checkout', $user, self::individual());

        self::assertSame(422, $response->status);
        self::assertSame(['error' => 'sale_ended'], self::body($response));
    }

    #[Test]
    public function aProductGoneByCheckoutIs422(): void
    {
        $user = '11111111-1111-1111-1111-11111111111e';
        $this->withdrawn($user, "DELETE FROM product WHERE product_id = '" . self::SECOND . "'");

        $response = $this->checkout('/carts/checkout', $user, self::individual());

        self::assertSame(422, $response->status);
        self::assertSame(['error' => 'product_not_found'], self::body($response));
    }

    #[Test]
    public function theFirstLineThatCannotBePricedSaysWhy(): void
    {
        // The lines are priced in the order of their products: the first ended its sale, the second
        // is gone, and the answer is the first's.
        $user = '11111111-1111-1111-1111-11111111111f';
        $this->withdrawn($user, "UPDATE product SET on_sale = 0 WHERE product_id = '" . self::ON_SALE . "'");
        $this->pdo->exec("DELETE FROM product WHERE product_id = '" . self::SECOND . "'");

        $response = $this->checkout('/carts/quote', $user, self::corporation());

        self::assertSame(422, $response->status);
        self::assertSame(['error' => 'sale_ended'], self::body($response));
    }

    /**
     * Puts the seeded product on sale and SECOND in the cart of $userId, then changes the products
     * by $change.
     */
    private function withdrawn(string $userId, string $change): void
    {
        $this->pdo->exec("INSERT INTO product (product_id, name, on_sale, price) VALUES ('"
            . self::SECOND . "', 'Filter Papers', 1, 300)");
        self::assertSame(201, $this->addItem($userId, self::ON_SALE, 1)->status);
        self::assertSame(201, $this->addItem($userId, self::SECOND, 1)->status);
        $this->pdo->exec($change);
    }

    private function addItem(string $userId, string $productId, int $quantity): Response
    {
        return $this->post('/carts/items', ['userId' => $userId, 'productId' => $productId, 'quantity' => $quantity]);
    }

    private function cartsOf(string $userId): int
    {
        $count = $this->pdo->prepare('SELECT COUNT(*) FROM cart WHERE user_id = ?');
        $count->execute([$userId]);
        return (int) $count->fetchColumn();
    }

    /** @param array<string, mixed> $orderer */
    private function checkout(string $path, string $userId, array $orderer): Response
    {
        return $this->post($path, ['userId' => $userId, 'orderer' => $orderer]);
    }

    /** @param array<string, mixed> $body */
    private function post(string $path, array $body): Response
    {
        return $this->app->handle(new Request('POST', $path, [], json_encode($body, JSON_THROW_ON_ERROR)));
    }

    /** @return array<string, mixed> */
    private static function body(Response $response): array
    {
        return json_decode((string) $response->body, true, flags: JSON_THROW_ON_ERROR);
    }

    /** @return array<string, string> */
    private static function individual(): array
    {
        return ['type' => 'Individual', 'email' => 'Taro@Example.com ', 'name' => '山田太郎'];
    }

    /** @return array<string, string> */
    private static function corporation(): array
    {
        return ['type' => 'Corporation', 'email' => 'info@acme.co.jp', 'companyName' => 'Acme株式会社',
            'corporateNumber' => '1234567890123'];
    }
}
