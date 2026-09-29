<?php
/**
 * What a runtime package offers a generated binding, as a listing one PHP version writes the same
 * as another.
 *
 * A listing that is a protocol's fingerprint has to be made of what the surface means and not of
 * how one PHP's Reflection spells it, since the spelling is not part of the surface and moves
 * between releases: `self` and `parent` are spelt as themselves by some and as the class they name
 * by others, a nullable type is `?T` or `T|null`, and a union's members come in an order PHP
 * chooses. A type is therefore written here as its members, each a class by its full name or a
 * built-in by its name, `self` and `parent` resolved to the class they stand for, `null` a member
 * of anything nullable, the members in order; `static` stays `static`, since late static binding
 * is part of what a method promises. What else is listed is sorted or is a set, so no order PHP
 * happens to give decides a line.
 *
 *   php php-runtime-surface.php surface <runtime directory>
 *   php php-runtime-surface.php types <file declaring the classes Fixture and Explicit>
 */

/** A type as its meaning, in declaring class `$class`. */
function canonical_type(?ReflectionType $type, ReflectionClass $class): string
{
    if ($type === null) {
        return '-';
    }
    $named = static function (ReflectionNamedType $it) use ($class): array {
        $name = $it->getName();
        $resolved = match (strtolower($name)) {
            'self' => $class->getName(),
            'parent' => $class->getParentClass() ? $class->getParentClass()->getName() : 'parent',
            default => $name,
        };
        return $it->allowsNull() && $resolved !== 'mixed' && $resolved !== 'null'
            ? [$resolved, 'null'] : [$resolved];
    };
    if ($type instanceof ReflectionNamedType) {
        $members = $named($type);
        $glue = '|';
    } elseif ($type instanceof ReflectionUnionType) {
        $members = [];
        foreach ($type->getTypes() as $member) {
            $members = array_merge($members, $member instanceof ReflectionNamedType
                ? $named($member) : [canonical_type($member, $class)]);
        }
        $glue = '|';
    } else {
        $members = array_map(static fn (ReflectionNamedType $it): string => $named($it)[0],
            $type->getTypes());
        $glue = '&';
    }
    $members = array_values(array_unique($members));
    sort($members);
    return implode($glue, $members);
}

/** Every public thing a class offers, one line each, in the class's own name. */
function class_lines(string $name, string $as): array
{
    $class = new ReflectionClass($name);
    $flags = static fn (array $held): string => implode(' ', array_keys(array_filter($held)));
    $interfaces = $class->getInterfaceNames();
    sort($interfaces);
    $lines = ["$as " . $flags(['interface' => $class->isInterface(),
            'abstract' => $class->isAbstract() && !$class->isInterface(),
            'final' => $class->isFinal(), 'readonly' => $class->isReadOnly(),
            'enum' => $class->isEnum()])
        . ' extends ' . ($class->getParentClass() ? $class->getParentClass()->getName() : '-')
        . ' implements ' . implode(',', $interfaces)];
    foreach ($class->getReflectionConstants(ReflectionClassConstant::IS_PUBLIC) as $it) {
        if ($it->getDeclaringClass()->getName() === $name) {
            $lines[] = "$as::{$it->getName()} constant";
        }
    }
    foreach ($class->getProperties(ReflectionProperty::IS_PUBLIC) as $it) {
        if ($it->getDeclaringClass()->getName() === $name) {
            $lines[] = "$as::\${$it->getName()} " . canonical_type($it->getType(), $class) . ' '
                . $flags(['static' => $it->isStatic(), 'readonly' => $it->isReadOnly()]);
        }
    }
    foreach ($class->getMethods(ReflectionMethod::IS_PUBLIC) as $it) {
        if ($it->getDeclaringClass()->getName() !== $name) {
            continue;
        }
        $parameters = array_map(static fn (ReflectionParameter $p): string =>
            canonical_type($p->getType(), $class) . ($p->isPassedByReference() ? ' &' : ' ')
            . ($p->isVariadic() ? '...' : '') . '$' . $p->getName()
            . ($p->isOptional() && !$p->isVariadic() ? ' =' : ''), $it->getParameters());
        $lines[] = "$as::{$it->getName()}(" . implode(', ', $parameters) . '): '
            . canonical_type($it->getReturnType(), $class) . ' '
            . $flags(['static' => $it->isStatic(), 'abstract' => $it->isAbstract(),
                'final' => $it->isFinal()]);
    }
    return $lines;
}

$mode = $argv[1] ?? '';
if ($mode === 'surface') {
    require $argv[2] . '/vendor/autoload.php';
    $lines = [];
    foreach (glob($argv[2] . '/src/*.php') as $file) {
        $name = 'Souther\\Runtime\\' . basename($file, '.php');
        $lines = array_merge($lines, class_lines($name, $name));
    }
    sort($lines);
    echo implode("\n", $lines), "\n";
} elseif ($mode === 'types') {
    require $argv[2];
    echo implode("\n", array_merge(class_lines('Fixture', 'C'), class_lines('Explicit', 'C'))), "\n";
} else {
    fwrite(STDERR, "surface <runtime directory> | types <file>\n");
    exit(2);
}
