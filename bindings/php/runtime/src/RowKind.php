<?php

declare(strict_types=1);

namespace Souther\Runtime;

/**
 * What a row of a behavior a host implements is, as far as running it goes: what decides what
 * evaluating it can come to ({@see Row}).
 */
enum RowKind
{
    /**
     * The row states an answer, and the library holds what an implementation answers to it: kept
     * or not kept ({@see RowKept}, {@see RowNotKept}), or ended without an answer.
     */
    case Asserted;

    /**
     * The row's answer is owed (`<?>`): it states nothing an answer could keep, so what an
     * implementation answers is only that ({@see RowAnswered}), with no verdict to read.
     */
    case Owed;

    /** The compile could not read the row, so there is nothing to run; {@see Row::$why} says why. */
    case Unreproducible;

    /**
     * The row states an answer the library has no way to compare an answer with, so there is
     * nothing to run either; {@see Row::$why} says so.
     */
    case NotComparable;
}
