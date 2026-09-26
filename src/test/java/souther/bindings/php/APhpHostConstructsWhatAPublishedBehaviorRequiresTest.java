package souther.bindings.php;

import souther.nativecode.Checked;
import org.junit.jupiter.api.Test;
import org.junit.jupiter.api.io.TempDir;
import souther.compiler.Compiler;
import souther.compiler.meta.ModulePath;
import souther.compiler.program.CheckedProgram;
import souther.nativecode.NativeCompiler;
import souther.nativecode.Php;
import tools.jackson.databind.JsonNode;
import tools.jackson.databind.json.JsonMapper;

import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.Path;
import java.util.List;

import static org.assertj.core.api.Assertions.assertThat;
import static org.assertj.core.api.Assertions.assertThatThrownBy;

/**
 * What a host calls and what a host constructs are two surfaces. A published behavior may depend
 * on one its module keeps (upstream ADR-0068: a callee arrives bound), which a host has no name for
 * and still builds a capability of wherever the published one is called. So the kept one has no
 * function and no class, and is constructed all the same.
 */
class APhpHostConstructsWhatAPublishedBehaviorRequiresTest {

    private static final String DEMO = """
            module demo exposing ( Price, outer )

            data Price = Int

            behavior listed : () -> Price

            behavior adjusted : () -> Price
                depends on listed
            let adjusted (listed) = listed()

            behavior outer : () -> Price
                depends on adjusted
            let outer (adjusted) = adjusted()
            """;

    private static final String HOST = """
            <?php
            declare(strict_types=1);

            require $argv[1] . '/vendor/autoload.php';
            require $argv[2] . '/autoload.php';

            $binding = \\Acme\\Binding::load($argv[3]);
            echo "outer: ", $binding->run(fn (): int => \\Acme\\Demo\\Behaviors::outer()->value(),
                \\Acme\\Demo\\Injections::of(
                    listed: fn (): \\Acme\\Demo\\Price => \\Acme\\Demo\\Price::of(42)->getOrThrow())), "\\n";
            echo "a class for what the module keeps: ",
                class_exists(\\Acme\\Demo\\Adjusted::class) ? "yes" : "no", "\\n";
            """;

    private static final Path RUNTIME = Path.of("bindings", "php", "runtime");

    @Test
    void aBehaviorTheModuleKeepsIsConstructedWhereAPublishedOneRequiresIt(@TempDir Path into)
            throws Exception {
        NativeCompiler.Library library = NativeCompiler.library(
                Checked.of(List.of(DEMO)), into.resolve("native"));
        PhpBindings.Generated binding =
                LibraryBinding.generated(library, into.resolve("php"), "Acme");
        Path host = into.resolve("host.php");
        Files.writeString(host, HOST, StandardCharsets.UTF_8);

        assertThat(Php.ran(List.of("-d", "ffi.enable=1", host.toString(),
                RUNTIME.toAbsolutePath().toString(), binding.root().toString(),
                library.library().toString()))).isEqualTo("""
                        outer: 42
                        a class for what the module keeps: no
                        """);

        JsonNode demo = JsonMapper.builder().build().readTree(library.manifest().toFile())
                .get("modules").get(0);
        assertThat(demo.get("behaviors").findValuesAsString("name")).doesNotContain("adjusted");
        assertThat(demo.get("constructions").findValuesAsString("name"))
                .contains("outer", "adjusted");
    }

    private static final String PORT = """
            module lib.port exposing ( lookUp, looked )

            behavior lookUp : (a: Int) -> Int

            behavior looked : (a: Int) -> Int
                depends on lookUp
            let looked (a, lookUp) = lookUp(a)
            """;

    private static final String CALLER = """
            module app.caller exposing ( twice )

            import lib.port ( looked )

            behavior twice : (a: Int) -> Int
                depends on looked
            let twice (a, looked) = looked(a) * 2
            """;

    /**
     * What a published behavior requires is reached through a capability and never named as a
     * symbol, so a library linked without the build that constructs it would link all the same, and
     * a host would find out when it built one. It is refused where the library is put together.
     */
    @Test
    void aLibraryWithoutWhatConstructsARequirementIsRefused(@TempDir Path into) {
        CheckedProgram caller = Checked.of(List.of(CALLER),
                ModulePath.of(Compiler.compile(PORT)));

        assertThatThrownBy(() -> NativeCompiler.library(caller, into))
                .hasMessageContaining("app.caller.twice requires lib.port.looked, which no object"
                        + " linked constructs");
    }
}
