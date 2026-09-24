<?php

declare(strict_types=1);

namespace Souther\Runtime;

/**
 * A library's runs and what is made in them used from a fiber other than the one they are going
 * on. Runs are one stack, ended in the order opposite the one they started in, and a fiber resumed
 * while another is suspended in a run would end them in some other order.
 */
final class RunOnAnotherFiber extends \LogicException implements SoutherFailure
{
}
