<?php

declare(strict_types=1);

namespace App\Http;

use Raoh\CallableDecoder;
use Raoh\Decoder;
use Raoh\Err;
use Raoh\Issue;
use Raoh\Issues;
use Raoh\Ok;
use Raoh\Path;
use Raoh\Result;

/**
 * A value the model reads whole, some of whose members the boundary puts in their canonical form
 * first: the email in lower case, a name without the spaces around it.
 *
 * What a value is belongs to the model, and how a client's text is written canonically belongs to
 * the boundary, so the two are separate steps over separate members. Each member is canonicalised
 * on its own, and the model reads the value whichever of them the boundary refused, with a refused
 * member as it was given; so a refused email does not keep the model from saying that a company
 * name is missing, as a `pipe` from the boundary's step into the model's would. Where both speak of
 * one member, the boundary's issue is the one kept: it says what form the member was not in, and
 * the model's could only say that the member did not hold.
 *
 * A `pipe` into a model's decoder is right only where the boundary's step is about the very value
 * the model reads, an id's UUID form: there the model's issue would be at the same path.
 */
final class Canonical
{
    /**
     * @param array<string, Decoder<mixed, mixed>> $members how each member is written canonically
     * @param Decoder<mixed, T> $model the model's decoder of the whole value
     * @return Decoder<mixed, T>
     * @template T
     */
    public static function of(array $members, Decoder $model): Decoder
    {
        return CallableDecoder::of(static function (mixed $given, ?Path $path = null) use ($members, $model): Result {
            $path ??= Path::root();
            $canonical = $given;
            $issues = Issues::empty();
            $refused = [];
            if (is_array($given)) {
                foreach ($members as $name => $form) {
                    if (!array_key_exists($name, $given)) {
                        continue;
                    }
                    $at = $path->append($name);
                    $written = $form->decode($given[$name], $at);
                    if ($written instanceof Ok) {
                        $canonical[$name] = $written->value;
                    } else {
                        \assert($written instanceof Err);
                        $refused[] = $at->segments();
                        $issues = $issues->merge($written->issues);
                    }
                }
            }
            $read = $model->decode($canonical, $path);
            if ($read instanceof Err) {
                foreach ($read->issues->toArray() as $issue) {
                    if (!self::spokenOf($issue, $refused)) {
                        $issues = $issues->add($issue);
                    }
                }
            }
            return $issues->isEmpty() ? $read : Result::err($issues);
        });
    }

    /** @param list<list<string>> $members */
    private static function spokenOf(Issue $issue, array $members): bool
    {
        $at = $issue->path->segments();
        foreach ($members as $member) {
            if (array_slice($at, 0, count($member)) === $member) {
                return true;
            }
        }
        return false;
    }
}
