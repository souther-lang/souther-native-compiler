<?php

declare(strict_types=1);

// php -d ffi.enable=1 -S localhost:8080 -t public public/index.php

require __DIR__ . '/../vendor/autoload.php';

use App\CartApplication;
use App\Http\Request;
use Model\Binding;

$database = getenv('CART_DATABASE') ?: __DIR__ . '/../build/cart.sqlite';

CartApplication::create(Binding::load(CartApplication::library()), new PDO('sqlite:' . $database))
    ->handle(Request::fromGlobals())
    ->send();
