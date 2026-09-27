<?php

declare(strict_types=1);

namespace App\Http;

use Raoh\CallableDecoder;
use Raoh\Decoder;
use Raoh\Err;
use Raoh\Issues;
use Raoh\Ok;
use Raoh\Path;
use Raoh\Result;

/**
 * A value the model reads whole, some of whose members the boundary owns: an orderer's email and
 * names.
 *
 * A raoh-php decoder writes a value in the form its reader wants and refuses what cannot be written
 * so: it canonicalises and validates as one step, and what it refuses has no value. Each member the
 * boundary owns is decoded on its own, and the model reads the value whichever of them was refused,
 * so the issues of both come back together; a refused email does not keep the model from saying
 * that a company name is missing, as a `pipe` from the boundary's decoder into the model's would.
 *
 * What the model is handed of a member is what the boundary's decoder answered for it, and of a
 * refused member nothing: the model never reads text the boundary refused, so what it sees does not
 * depend on whether the rest of the request was valid. It then reports the refused member as
 * missing, which is only that it was taken out, and that one issue is dropped: the one at the
 * member's own path. Nothing inside the member is the model's to report, since the model was not
 * handed it; what is wrong inside it is its decoder's to say.
 */
final class Members
{
    /**
     * @param array<string, Decoder<mixed, mixed>> $members the boundary's decoder of each member it owns
     * @param Decoder<mixed, T> $model the model's decoder of the whole value
     * @return Decoder<mixed, T>
     * @template T
     */
    public static function of(array $members, Decoder $model): Decoder
    {
        return CallableDecoder::of(static function (mixed $given, ?Path $path = null) use ($members, $model): Result {
            $path ??= Path::root();
            $decoded = $given;
            $issues = Issues::empty();
            $refused = [];
            if (is_array($given)) {
                foreach ($members as $name => $decoder) {
                    if (!array_key_exists($name, $given)) {
                        continue;
                    }
                    $at = $path->append($name);
                    $written = $decoder->decode($given[$name], $at);
                    if ($written instanceof Ok) {
                        $decoded[$name] = $written->value;
                    } else {
                        \assert($written instanceof Err);
                        unset($decoded[$name]);
                        $refused[] = $at->segments();
                        $issues = $issues->merge($written->issues);
                    }
                }
            }
            $read = $model->decode($decoded, $path);
            if ($read instanceof Err) {
                foreach ($read->issues->toArray() as $issue) {
                    if (!in_array($issue->path->segments(), $refused, true)) {
                        $issues = $issues->add($issue);
                    }
                }
            }
            return $issues->isEmpty() ? $read : Result::err($issues);
        });
    }
}
