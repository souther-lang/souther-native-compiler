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

    private CartApplication $app;

    protected function setUp(): void
    {
        $this->app = CartApplication::create(
            Binding::load(CartApplication::library()), new PDO('sqlite::memory:'));
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
    public function anOrdererOfNoKnownTypeIs400(): void
    {
        $response = $this->checkout('/carts/checkout', self::USER, ['type' => 'Robot'] + self::individual());

        self::assertSame(400, $response->status);
        self::assertSame('/orderer/type', self::body($response)['issues'][0]['path']);
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

    private function addItem(string $userId, string $productId, int $quantity): Response
    {
        return $this->post('/carts/items', ['userId' => $userId, 'productId' => $productId, 'quantity' => $quantity]);
    }

    /** @param array<string, string> $orderer */
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
