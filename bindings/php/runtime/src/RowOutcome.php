<?php

declare(strict_types=1);

namespace Souther\Runtime;

/**
 * What evaluating one row against one implementation came to ({@see Row::evaluate()}): the answer
 * kept the row ({@see RowKept}), or did not ({@see RowNotKept}); the row's answer is owed and the
 * implementation answered, with no verdict ({@see RowAnswered}); the row's run ended without an
 * answer ({@see RowEnded}); or there was nothing to run ({@see RowNotRun}).
 *
 * Five classes and not one with parts that may be missing, so that an owed row has no verdict to
 * read and a run that ended has no answer to compare. Which of them fails a test is the test's to
 * say: a row with nothing to run may be one it skips or one it fails, and an owed row's answer is
 * one it may show and holds to nothing.
 */
interface RowOutcome
{
}
