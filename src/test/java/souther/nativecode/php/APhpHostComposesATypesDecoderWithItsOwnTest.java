package souther.nativecode.php;

import org.junit.jupiter.api.Test;
import org.junit.jupiter.api.io.TempDir;
import souther.compiler.program.CheckedProgram;
import souther.nativecode.NativeCompiler;
import souther.nativecode.Php;

import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.Path;
import java.util.List;

import static org.assertj.core.api.Assertions.assertThat;

/**
 * A PHP host composes a type's decoder with raoh-php's own, as a JVM host composes a type's
 * {@code decoder()}: raoh-php checks the form of what came in and normalises it, and the model's
 * decoder reads what it handed on as the type, checking what the type states and telling a sum's
 * cases apart.
 *
 * <p>What the model finds wrong is an issue at the path the decoder was reached at, beside raoh's
 * own, so a host answers both the same way. The cart #58 ports decodes its orderer like this.
 */
class APhpHostComposesATypesDecoderWithItsOwnTest {

    private static final String ORDERING = """
            module ordering exposing ( Email, Individual, Corporation, Orderer, Quantity, Line, Lines )

            data Email = String
                invariant String.length(value) >= 3
            data Individual = { email: Email, name: String }
            data Corporation = { email: Email, companyName: String }
            data Orderer = Individual | Corporation

            data Quantity = Int
                invariant value > 0
            data Line = { sku: String, quantity: Quantity }
            data Lines = { lines: List<Line> }
            """;

    private static final String HOST = """
            <?php
            declare(strict_types=1);

            require $argv[1] . '/vendor/autoload.php';
            require $argv[2] . '/autoload.php';

            use Acme\\Shop\\Binding;
            use Acme\\Shop\\Ordering\\Corporation;
            use Acme\\Shop\\Ordering\\Individual;
            use Acme\\Shop\\Ordering\\Lines;
            use Acme\\Shop\\Ordering\\OrdererCodec;
            use Acme\\Shop\\Ordering\\Quantity;
            use Raoh\\Issue;
            use Raoh\\Result;

            use function Raoh\\Boundary\\Json\\combine;
            use function Raoh\\Boundary\\Json\\field;
            use function Raoh\\Boundary\\Json\\from_json;
            use function Raoh\\Boundary\\Json\\string_;

            function said(Result $result): string {
                return $result->fold(
                    fn (mixed $value): string => 'ok ' . (new ReflectionClass($value))->getShortName(),
                    fn ($issues): string => implode(' ', array_map(
                        fn (Issue $it): string => '[' . ($it->path->toJsonPointer() ?: '/') . ' ' . $it->code . ']',
                        $issues->toArray())));
            }

            // A decoder holds no run: made once, it reads in whichever run it is used in.
            $quantity = Quantity::decoder();

            echo Binding::load($argv[3])->run(function () use ($quantity): string {
                // The model's decoder reads the orderer: which case it is by `type`, its fields, and
                // what Email states.
                $checkout = from_json(field('orderer', OrdererCodec::decoder()));
                $out = [];
                $out[] = 'individual: ' . said($checkout->decode(
                    '{"orderer":{"type":"Individual","email":"a@b","name":"Taro"}}'));
                $individual = $checkout->decode('{"orderer":{"type":"Individual","email":"a@b","name":"Taro"}}')
                    ->getOrThrow();
                $out[] = 'read: ' . $individual->email()->value() . ' ' . $individual->name();
                $out[] = 'corporation: ' . said($checkout->decode(
                    '{"orderer":{"type":"Corporation","email":"x@y","companyName":"Acme"}}'));
                $out[] = 'short email: ' . said($checkout->decode(
                    '{"orderer":{"type":"Individual","email":"ab","name":"Taro"}}'));
                $out[] = 'no such case: ' . said($checkout->decode('{"orderer":{"type":"Robot"}}'));
                $out[] = 'missing field: ' . said($checkout->decode(
                    '{"orderer":{"type":"Corporation","email":"x@y"}}'));

                // A field of the host's own beside one of the model's: both are checked, and each
                // issue is at its own path.
                $both = combine(
                    field('who', string_()->minLength(2)),
                    field('individual', Individual::decoder()),
                )->map(fn (string $who, Individual $individual): Individual => $individual);
                $out[] = 'both: ' . said($both->decode(['who' => 'x', 'individual' => ['email' => 'ab', 'name' => 'T']]));

                // A newtype reads its value, and a list its elements, each at its index.
                $out[] = 'quantity: ' . said(field('n', Quantity::decoder())->decode(['n' => 0]));
                $out[] = 'a float: ' . said(Quantity::decoder()->decode(2.0));
                $out[] = 'lines: ' . said(Lines::decoder()->decode(
                    ['lines' => [['sku' => 'a', 'quantity' => 1], ['sku' => 'b', 'quantity' => 0]]]));
                $out[] = 'no lines: ' . said(Lines::decoder()->decode(['lines' => []]));
                $out[] = 'not json: ' . said(Quantity::decoder()->decode(NAN));
                $out[] = 'made before the run: ' . said($quantity->decode(3));
                $out[] = 'case alone: ' . said(Corporation::decoder()->decode(
                    ['email' => 'x@y', 'companyName' => 'Acme']));
                return implode("\\n", $out);
            }), "\\n";
            """;

    private static final String ANSWERED = """
            individual: ok Individual
            read: a@b Taro
            corporation: ok Corporation
            short email: [/orderer/email invariant_violation]
            no such case: [/orderer/type not_allowed]
            missing field: [/orderer/companyName missing_field]
            both: [/who too_short] [/individual/email invariant_violation]
            quantity: [/n invariant_violation]
            a float: [/ type_mismatch]
            lines: [/lines/1/quantity invariant_violation]
            no lines: ok Lines
            not json: [/ type_mismatch]
            made before the run: ok Quantity
            case alone: ok Corporation
            """;

    private static final Path RUNTIME = Path.of("bindings", "php", "runtime");

    @Test
    void aTypesDecoderComposesWithRaohsAndReportsAtItsPath(@TempDir Path into) throws Exception {
        NativeCompiler.Library library =
                NativeCompiler.library(CheckedProgram.of(List.of(ORDERING)), into.resolve("native"));
        PhpBindings.Generated binding =
                PhpBindings.generate(library, into.resolve("php"), "Acme\\Shop");
        Path host = into.resolve("host.php");
        Files.writeString(host, HOST, StandardCharsets.UTF_8);

        assertThat(Php.ran(List.of("-d", "ffi.enable=1", host.toString(),
                RUNTIME.toAbsolutePath().toString(), binding.root().toString(),
                library.library().toString()))).isEqualTo(ANSWERED);
    }
}
