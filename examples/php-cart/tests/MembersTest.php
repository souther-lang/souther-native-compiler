<?php

declare(strict_types=1);

namespace App\Tests;

use App\Http\Members;
use PHPUnit\Framework\Attributes\Test;
use PHPUnit\Framework\TestCase;
use Raoh\CallableDecoder;
use Raoh\Decoder;
use Raoh\Err;
use Raoh\Issues;
use Raoh\Path;
use Raoh\Result;

use function Raoh\Boundary\Json\string_;

/**
 * How the boundary's decoders and the model's are put together over one value, with a model that
 * records what it was handed, so what it reads is seen and not inferred from what it answers.
 */
final class MembersTest extends TestCase
{
    /** @var list<mixed> */
    private array $read = [];

    #[Test]
    public function theModelReadsWhatTheBoundaryWrote(): void
    {
        Members::of(['email' => self::email()], $this->recording(['email']))
            ->decode(['email' => ' A@Example.COM ']);

        self::assertSame([['email' => 'a@example.com']], $this->read);
    }

    #[Test]
    public function theModelIsNeverHandedWhatTheBoundaryRefused(): void
    {
        $read = Members::of(['email' => self::email()], $this->recording(['email', 'city']))
            ->decode(['email' => ' X ', 'city' => 'Tokyo']);

        self::assertSame([['city' => 'Tokyo']], $this->read);
        self::assertSame(['/email'], self::paths($read));
    }

    #[Test]
    public function theModelReadsTheRestWhicheverMemberWasRefused(): void
    {
        $read = Members::of(['email' => self::email()], $this->recording(['email', 'city']))
            ->decode(['email' => 'not-an-email']);

        self::assertSame(['/email', '/city'], self::paths($read));
    }

    #[Test]
    public function onlyTheRefusedMembersOwnPathIsTakenForTheBoundarys(): void
    {
        // The boundary's decoder of an address refuses its postcode, and the address is taken out.
        // The model reports it missing, which is dropped, and the model's own issue beside it is
        // kept: nothing at a path merely under or beside the refused member is dropped.
        $address = CallableDecoder::of(fn (mixed $given, ?Path $path = null): Result =>
            Result::fail(($path ?? Path::root())->append('postcode'), 'invalid_format', 'not a postcode'));

        $read = Members::of(['address' => $address], $this->recording(['address', 'addressee']))
            ->decode(['address' => ['postcode' => 1]]);

        self::assertSame(['/address/postcode', '/addressee'], self::paths($read));
    }

    /** @return Decoder<mixed, string> */
    private static function email(): Decoder
    {
        return string_()->trim()->toLowerCase()->email();
    }

    /**
     * A model that records what it was handed, and reports each of `$required` it is missing.
     *
     * @param list<string> $required
     * @return Decoder<mixed, null>
     */
    private function recording(array $required): Decoder
    {
        return CallableDecoder::of(function (mixed $given, ?Path $path = null) use ($required): Result {
            $this->read[] = $given;
            $path ??= Path::root();
            $missing = Issues::empty();
            foreach ($required as $name) {
                if (!array_key_exists($name, $given)) {
                    $missing = $missing->merge(
                        Result::fail($path->append($name), 'missing_field', 'field is missing')->issues);
                }
            }
            return $missing->isEmpty() ? Result::ok(null) : Result::err($missing);
        });
    }

    /** @return list<string> */
    private static function paths(Result $read): array
    {
        self::assertInstanceOf(Err::class, $read);
        return array_map(fn ($issue): string => $issue->path->toJsonPointer(), $read->issues->toArray());
    }
}
