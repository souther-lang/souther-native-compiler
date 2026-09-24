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
import static org.assertj.core.api.Assertions.assertThatThrownBy;

/**
 * What the generator writes from a manifest, and what it refuses to, asked of the PHP it writes
 * rather than of a run.
 */
class APhpBindingIsWrittenFromTheManifestTest {

    private static PhpBindings.Generated generated(Path into, String source) throws Exception {
        NativeCompiler.Library library =
                NativeCompiler.library(CheckedProgram.of(List.of(source)), into.resolve("native"));
        return PhpBindings.generate(library, into.resolve("php"), "Acme\\Billing");
    }

    private static String behaviors(PhpBindings.Generated generated) throws Exception {
        return Files.readString(generated.root().resolve("M").resolve("Behaviors.php"));
    }

    @Test
    void everyFileItWritesIsPhp(@TempDir Path into) throws Exception {
        PhpBindings.Generated generated = generated(into, """
                module m exposing ( Found, Missing, Lookup, Box, find, open, amount )

                data Found = { id: Int, label: String? }
                data Missing
                data Lookup = Found | Missing
                data Box = Bool

                behavior find : (id: Int) -> Lookup
                let find (id) = if id > 0 then Found { id = id, label = None } else Missing

                behavior open : (box: Box, lookup: Lookup) -> Bool
                let open (box, lookup) = box.value

                behavior priceOf : (id: Int) -> Int

                let amount = Box(true)
                """);

        for (Path file : generated.files()) {
            if (file.toString().endsWith(".php")) {
                assertThat(Php.compiles(file)).as("%s", file).isTrue();
            }
        }
        assertThat(generated.files()).extracting(it -> generated.root().relativize(it).toString())
                .contains("Binding.php", "autoload.php", "souther.ffi.h", "M/Found.php",
                        "M/Missing.php", "M/Lookup.php", "M/LookupCodec.php", "M/Box.php",
                        "M/Behaviors.php", "M/Values.php", "M/Injections.php")
                // Every case of Lookup has a class, and the library says which a value is.
                .doesNotContain("M/LookupValue.php");
    }

    /** Where the manifest gives a behavior no way in, nothing is written that a caller could call. */
    @Test
    void aBehaviorTheManifestGivesNoCallIsNotWritten(@TempDir Path into) throws Exception {
        NativeCompiler.Library library = NativeCompiler.library(CheckedProgram.of(List.of("""
                module m exposing ( half, twice )

                behavior half : (n: Int) -> Int
                let half (n) = n

                behavior twice : (n: Int) -> Int
                let twice (n) = n * 2
                """)), into.resolve("native"));
        Path manifest = into.resolve("unreachable.json");
        String written = Files.readString(library.manifest());
        int half = written.indexOf("\"name\": \"half\"");
        int call = written.indexOf("\"call\": {", half);
        int closed = written.indexOf("\n          }", call);
        Files.writeString(manifest, written.substring(0, call) + "\"call\": null"
                + written.substring(closed + "\n          }".length()), StandardCharsets.UTF_8);

        PhpBindings.generate(manifest, library.declarations(), into.resolve("php"), "Acme\\Billing");

        assertThat(Files.readString(into.resolve("php").resolve("M").resolve("Behaviors.php")))
                .contains("function twice(").doesNotContain("half");
    }

    /**
     * A behavior answering a union no declaration names is not written: the library says of no such
     * value which of its cases it is, and an answer a host cannot tell apart is not one it can use.
     */
    @Test
    void aBehaviorAnsweringAnUnnamedUnionIsNotWritten(@TempDir Path into) throws Exception {
        String written = behaviors(generated(into, """
                module m exposing ( Found, Missing, find, twice )

                data Found = { id: Int }
                data Missing

                behavior find : (id: Int) -> Found | Missing
                let find (id) = if id > 0 then Found { id = id } else Missing

                behavior twice : (n: Int) -> Int
                let twice (n) = n * 2
                """));

        assertThat(written).contains("function twice(").doesNotContain("find");
    }

    @Test
    void aTypeNamedAsAClassTheBindingWritesIsRefused(@TempDir Path into) {
        assertThatThrownBy(() -> generated(into, """
                module m exposing ( Behaviors )

                data Behaviors = Int
                """))
                .isInstanceOf(PhpBindings.NotBindable.class)
                .hasMessageContaining("type `m.Behaviors`");
    }

    /** The session a call takes is named so that no parameter of the model's is renamed for it. */
    @Test
    void theSessionGivesWayToAParameterOfTheSameName(@TempDir Path into) throws Exception {
        String written = behaviors(generated(into, """
                module m exposing ( renew )

                behavior renew : (session: Int, ffi: Int) -> Int
                let renew (session, ffi) = session + ffi
                """));

        assertThat(written).contains(
                "renew(\\Souther\\Runtime\\Session $session_, int $session, int $ffi): int",
                "$ffi_ = $session_->call();");
    }

    @Test
    void aTypePhpReservesTheNameOfIsRefused(@TempDir Path into) {
        assertThatThrownBy(() -> generated(into, """
                module m exposing ( Match )

                data Match = Int
                """))
                .isInstanceOf(PhpBindings.NotBindable.class)
                .hasMessageContaining("`Match` is a word PHP reserves");
    }

    @Test
    void aFieldNamedAsAMethodTheBindingWritesIsRefused(@TempDir Path into) {
        assertThatThrownBy(() -> generated(into, """
                module m exposing ( Tag )

                data Tag = { encode: Bool }
                """))
                .isInstanceOf(PhpBindings.NotBindable.class)
                .hasMessageContaining("field `encode`")
                .hasMessageContaining("one name to PHP");
    }

    @Test
    void aNamespaceIsTheBindingsOwnAndMustBeOne(@TempDir Path into) throws Exception {
        NativeCompiler.Library library = NativeCompiler.library(CheckedProgram.of(List.of("""
                module m exposing ( Box )

                data Box = Bool
                """)), into.resolve("native"));

        assertThatThrownBy(() -> PhpBindings.generate(library, into.resolve("php"), ""))
                .isInstanceOf(PhpBindings.NotBindable.class);
        assertThatThrownBy(() -> PhpBindings.generate(library, into.resolve("php"), "Acme\\Class"))
                .isInstanceOf(PhpBindings.NotBindable.class)
                .hasMessageContaining("`Class` is a word PHP reserves");
    }

    /**
     * A manifest of a version this was not written for is refused as that, and not as whichever
     * member moved since: version 2, as the driver wrote it, said what a behavior takes as
     * {@code takes}.
     */
    @Test
    void aManifestOfAnotherVersionIsRefusedByItsVersion(@TempDir Path into) throws Exception {
        Path earlier = Path.of("src", "test", "resources", "souther", "nativecode", "php",
                "interface-v2.json");
        Path declarations = into.resolve("souther.declarations");
        Files.writeString(declarations, "", StandardCharsets.UTF_8);

        assertThatThrownBy(() -> PhpBindings.generate(earlier, declarations, into.resolve("php"),
                "Acme\\Billing"))
                .isInstanceOf(IllegalArgumentException.class)
                .hasMessageContaining("is version 2 of souther-native-interface for ABI generation"
                        + " 3, and this generator reads version 3")
                .hasMessageNotContaining("takes");
    }

    /**
     * What generated code calls of the runtime is the version the runtime says it is: the two are
     * written in two languages, and a binding refuses to load over a runtime of another version.
     */
    @Test
    void theRuntimeSpeaksTheVersionABindingIsWrittenFor() throws Exception {
        Path binding = Path.of("bindings", "php", "runtime", "src", "Binding.php");

        assertThat(Php.ran(List.of("-r", "require '" + binding.toAbsolutePath()
                + "'; echo \\Souther\\Runtime\\Binding::PROTOCOL;")))
                .isEqualTo(String.valueOf(PhpBindings.RUNTIME_PROTOCOL));
    }

    /** A parameter the model names as PHP names a superglobal is one no PHP function can have. */
    @Test
    void aParameterNamedAsASuperglobalIsRefused(@TempDir Path into) {
        assertThatThrownBy(() -> generated(into, """
                module m exposing ( f )

                behavior f : (GLOBALS: Int) -> Int
                let f (x) = x
                """))
                .isInstanceOf(PhpBindings.NotBindable.class)
                .hasMessageContaining("`GLOBALS`, which PHP takes for no parameter");
    }

    /** Two parameters of one function under one name would be PHP no binding can load. */
    @Test
    void twoParametersUnderOneNameAreRefused(@TempDir Path into) {
        assertThatThrownBy(() -> generated(into, """
                module m exposing ( f )

                behavior f : (a: Int, a: Int) -> Int
                let f (x, y) = x
                """))
                .isInstanceOf(PhpBindings.NotBindable.class)
                .hasMessageContaining("parameter `a`")
                .hasMessageContaining("PHP takes for one parameter");
    }

    /** A directory a binding is written to is that binding, and nothing a model had before. */
    @Test
    void aTypeTheModelNoLongerDeclaresLeavesTheBinding(@TempDir Path into) throws Exception {
        Path php = into.resolve("php");
        PhpBindings.generate(NativeCompiler.library(CheckedProgram.of(List.of("""
                module m exposing ( Kept, Dropped )

                data Kept = Int
                data Dropped = Bool
                """)), into.resolve("before")), php, "Acme\\Billing");
        assertThat(php.resolve("M").resolve("Dropped.php")).exists();

        PhpBindings.generate(NativeCompiler.library(CheckedProgram.of(List.of("""
                module m exposing ( Kept )

                data Kept = Int
                """)), into.resolve("after")), php, "Acme\\Billing");

        assertThat(php.resolve("M").resolve("Kept.php")).exists();
        assertThat(php.resolve("M").resolve("Dropped.php")).doesNotExist();
    }

    /** A generation refused part of the way leaves what was there, and nothing beside it. */
    @Test
    void aRefusedGenerationLeavesTheBindingThatWasThere(@TempDir Path into) throws Exception {
        Path php = into.resolve("php");
        PhpBindings.generate(NativeCompiler.library(CheckedProgram.of(List.of("""
                module m exposing ( Kept )

                data Kept = Int
                """)), into.resolve("before")), php, "Acme\\Billing");
        String before = Files.readString(php.resolve("M").resolve("Kept.php"));
        NativeCompiler.Library refused = NativeCompiler.library(CheckedProgram.of(List.of("""
                module m exposing ( Kept, Tag )

                data Kept = Bool
                data Tag = { encode: Bool }
                """)), into.resolve("after"));

        assertThatThrownBy(() -> PhpBindings.generate(refused, php, "Acme\\Billing"))
                .isInstanceOf(PhpBindings.NotBindable.class);
        assertThat(Files.readString(php.resolve("M").resolve("Kept.php"))).isEqualTo(before);
        try (var beside = Files.list(into)) {
            assertThat(beside.map(it -> it.getFileName().toString()))
                    .containsExactlyInAnyOrder("php", "before", "after");
        }
    }

    /** A directory holding what no generation wrote is not replaced, and keeps what it holds. */
    @Test
    void aDirectoryABindingDidNotWriteIsNotReplaced(@TempDir Path into) throws Exception {
        Path php = Files.createDirectories(into.resolve("php"));
        Files.writeString(php.resolve("mine.php"), "<?php\n", StandardCharsets.UTF_8);

        assertThatThrownBy(() -> PhpBindings.generate(NativeCompiler.library(
                CheckedProgram.of(List.of("""
                        module m exposing ( Kept )

                        data Kept = Int
                        """)), into.resolve("native")), php, "Acme\\Billing"))
                .isInstanceOf(PhpBindings.NotBindable.class)
                .hasMessageContaining("holds files a binding did not write");
        assertThat(php.resolve("mine.php")).exists();
    }
}
