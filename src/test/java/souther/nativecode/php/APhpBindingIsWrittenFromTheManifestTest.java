package souther.nativecode.php;

import org.junit.jupiter.api.Test;
import org.junit.jupiter.api.io.TempDir;
import souther.compiler.program.CheckedProgram;
import souther.nativecode.NativeCompiler;
import souther.nativecode.Php;
import tools.jackson.databind.JsonNode;
import tools.jackson.databind.json.JsonMapper;
import tools.jackson.databind.node.ArrayNode;
import tools.jackson.databind.node.ObjectNode;

import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.Path;
import java.util.List;
import java.util.function.Consumer;

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
     * A behavior answering a union no declaration names answers it as the union of its members'
     * classes, each value made as the class of the case the library says it is. Nothing is written
     * for the union itself: it has no name, and a class for it would be one the model does not have.
     */
    @Test
    void aBehaviorAnsweringAnUnnamedUnionAnswersTheClassOfItsCase(@TempDir Path into)
            throws Exception {
        PhpBindings.Generated generated = generated(into, """
                module m exposing ( Found, Missing, find )

                data Found = { id: Int }
                data Missing

                behavior find : (id: Int) -> Found | Missing
                let find (id) = if id > 0 then Found { id = id } else Missing
                """);
        String written = behaviors(generated);

        assertThat(written).contains(
                "find(\\Souther\\Runtime\\Session $session, int $id):"
                        + " \\Acme\\Billing\\M\\Found|\\Acme\\Billing\\M\\Missing",
                "$session->ffi()->souther3_m_m_b_find_answer_case($answer)",
                "0 => new \\Acme\\Billing\\M\\Found($session->held($answer))",
                "1 => new \\Acme\\Billing\\M\\Missing($session->held($answer))");
        assertThat(generated.files()).extracting(it -> generated.root().relativize(it).toString())
                .containsExactlyInAnyOrder("Binding.php", "autoload.php", "souther.ffi.h",
                        "M/Found.php", "M/Missing.php", "M/Behaviors.php", "M/Find.php");
    }

    /**
     * A union no declaration names that a host answers is handed over as the object PHP holds,
     * which already is the case it is: the implementation is typed as answering one of the
     * members' classes, and held to it.
     */
    @Test
    void anUnnamedUnionAHostAnswersIsHandedOverAsItIs(@TempDir Path into) throws Exception {
        PhpBindings.Generated generated = generated(into, """
                module m exposing ( Found, Missing )

                data Found = { id: Int }
                data Missing

                behavior lookUp : (id: Int) -> Found | Missing
                """);

        assertThat(Files.readString(generated.root().resolve("M").resolve("Injections.php")))
                .contains("@param (callable(\\Souther\\Runtime\\Session, int):"
                        + " \\Acme\\Billing\\M\\Found|\\Acme\\Billing\\M\\Missing)|null $lookUp");
        assertThat(Files.readString(generated.root().resolve("Binding.php"))).contains(
                "($answer instanceof \\Acme\\Billing\\M\\Found"
                        + " || $answer instanceof \\Acme\\Billing\\M\\Missing)",
                "$answer->nativeHandle()->borrow($session)");
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

    /**
     * A behavior is written as a class named after it, which stands among the classes the binding
     * writes for its module: one PHP takes for another of them is refused, and not renamed. The
     * checker already refuses one named as a type.
     */
    @Test
    void aBehaviorNamedAsAClassTheBindingWritesIsRefused(@TempDir Path into) {
        assertThatThrownBy(() -> generated(into, """
                module m exposing ( behaviors )

                behavior behaviors : (n: Int) -> Int
                let behaviors (n) = n
                """))
                .isInstanceOf(PhpBindings.NotBindable.class)
                .hasMessageContaining("the generated `Behaviors`")
                .hasMessageContaining("the class of behavior `m.behaviors`");
    }

    @Test
    void aBehaviorWhoseClassPhpReservesTheNameOfIsRefused(@TempDir Path into) {
        assertThatThrownBy(() -> generated(into, """
                module m exposing ( clone )

                behavior clone : (n: Int) -> Int
                let clone (n) = n
                """))
                .isInstanceOf(PhpBindings.NotBindable.class)
                .hasMessageContaining("behavior `m.clone` `Clone` is a word PHP reserves");
    }

    /**
     * A behavior requiring one the manifest gives a host no way to implement has no class, since
     * binding it could not be handed what it requires, and neither has what requires it in turn.
     * Each is still called as a function, with what a run registers.
     */
    @Test
    void aBehaviorRequiringWhatNoHostCanImplementHasNoClass(@TempDir Path into) throws Exception {
        NativeCompiler.Library library = NativeCompiler.library(CheckedProgram.of(List.of("""
                module m exposing ( charge, charged, twice )

                behavior rate : (n: Int) -> Int

                behavior charge : (n: Int) -> Int
                    depends on rate
                let charge (n, rate) = rate(n)

                behavior charged : (n: Int) -> Int
                    depends on charge
                let charged (n, charge) = charge(n) + 1

                behavior twice : (n: Int) -> Int
                let twice (n) = n * 2
                """)), into.resolve("native"));

        generatedAfter(into, library, "m", module -> ((ArrayNode) module.get("injections")).removeAll());

        Path written = into.resolve("php").resolve("M");
        assertThat(written.resolve("Twice.php")).exists();
        assertThat(written.resolve("Charge.php")).doesNotExist();
        assertThat(written.resolve("Charged.php")).doesNotExist();
        assertThat(Files.readString(written.resolve("Behaviors.php")))
                .contains("function charge(", "function charged(");
    }

    /**
     * What binding a behavior takes is named after each behavior it requires, so two requirements
     * of one name from two modules would be one parameter. Refused rather than named after their
     * modules too: the names would be this generator's, and the model says none.
     */
    @Test
    void twoRequirementsOfOneNameAreRefused(@TempDir Path into) {
        assertThatThrownBy(() -> PhpBindings.generate(NativeCompiler.library(
                CheckedProgram.of(List.of("""
                        module a exposing ( load )

                        behavior load : (n: Int) -> Int
                        """, """
                        module b exposing ( load )

                        behavior load : (n: Int) -> Int
                        """, """
                        module m exposing ( both : Int )

                        import a
                        import b

                        behavior both = a.load >-> b.load
                        """)), into.resolve("native")), into.resolve("php"), "Acme\\Billing"))
                .isInstanceOf(PhpBindings.NotBindable.class)
                .hasMessageContaining("behavior `a.load`, which `m.both` requires")
                .hasMessageContaining("behavior `b.load`, which `m.both` requires");
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
     * member moved since: version 3, as the driver wrote it, said what a behavior answers as a
     * type.
     */
    @Test
    void aManifestOfAnotherVersionIsRefusedByItsVersion(@TempDir Path into) throws Exception {
        Path earlier = Path.of("src", "test", "resources", "souther", "nativecode", "php",
                "interface-v3.json");
        Path declarations = into.resolve("souther.declarations");
        Files.writeString(declarations, "", StandardCharsets.UTF_8);

        assertThatThrownBy(() -> PhpBindings.generate(earlier, declarations, into.resolve("php"),
                "Acme\\Billing"))
                .isInstanceOf(IllegalArgumentException.class)
                .hasMessageContaining("is version 3 of souther-native-interface for ABI generation"
                        + " 3, and this generator reads version 6")
                .hasMessageNotContaining("answers");
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

    /** Two modules each holding a list of values of a type of its own. */
    private static final String TWO_MODULES_OF_LISTS = """
            module shop exposing ( Item, Cart )

            data Item = { n: Int }
            data Cart = { items: List<Item> }
            """;

    private static final String SECOND_MODULE_OF_LISTS = """
            module stock exposing ( Part, Bin )

            data Part = { n: Int }
            data Bin = { parts: List<Part> }
            """;

    private static NativeCompiler.Library twoModules(Path into) throws Exception {
        return NativeCompiler.library(CheckedProgram.of(List.of(TWO_MODULES_OF_LISTS,
                SECOND_MODULE_OF_LISTS)), into.resolve("native"));
    }

    private static final JsonMapper JSON = JsonMapper.builder().build();

    /** Generates from the library's manifest after {@code changing} the module named {@code module}. */
    private static void generatedAfter(Path into, NativeCompiler.Library library, String module,
                                       Consumer<ObjectNode> changing) throws Exception {
        JsonNode manifest = JSON.readTree(library.manifest().toFile());
        for (JsonNode it : manifest.get("modules")) {
            if (it.get("name").stringValue().equals(module)) {
                changing.accept((ObjectNode) it);
            }
        }
        Path changed = into.resolve("changed.json");
        Files.writeString(changed, JSON.writeValueAsString(manifest), StandardCharsets.UTF_8);
        PhpBindings.generate(changed, library.declarations(), into.resolve("php"), "Acme\\Billing");
    }

    /**
     * A list is built and read through the functions of the module whose function hands it across,
     * though another module's for the same element would do the same: those are what that module's
     * object offers, and the binding of one module does not reach into another's.
     */
    @Test
    void aListIsBuiltThroughItsOwnModulesFunctions(@TempDir Path into) throws Exception {
        PhpBindings.Generated generated =
                PhpBindings.generate(twoModules(into), into.resolve("php"), "Acme\\Billing");

        assertThat(Files.readString(generated.root().resolve("Shop").resolve("Cart.php")))
                .contains("souther3_m_shop_l_value_construct", "souther3_m_shop_l_value_at")
                .doesNotContain("souther3_m_stock_");
        assertThat(Files.readString(generated.root().resolve("Stock").resolve("Bin.php")))
                .contains("souther3_m_stock_l_value_construct", "souther3_m_stock_l_value_at")
                .doesNotContain("souther3_m_shop_");
    }

    /**
     * Every list a module says is held to what a list of its element is built and read through,
     * whether or not another module says a good one for the same element.
     */
    @Test
    void aListAModuleSaysOtherThanAListIsRefused(@TempDir Path into) throws Exception {
        NativeCompiler.Library library = twoModules(into);

        assertThatThrownBy(() -> generatedAfter(into, library, "stock", module -> {
            ObjectNode at = (ObjectNode) module.get("lists").get(0).get("at");
            ((ArrayNode) at.get("takes")).remove(2);
        }))
                .isInstanceOf(IllegalStateException.class)
                .hasMessageContaining("souther3_m_stock_l_value_at");
    }

    /**
     * A module with a function handing a list across and nothing to build one through is the
     * manifest and the binding disagreeing, and is refused rather than written without the function.
     */
    @Test
    void aListWithNothingToBuildItThroughIsRefused(@TempDir Path into) throws Exception {
        NativeCompiler.Library library = twoModules(into);

        assertThatThrownBy(() -> generatedAfter(into, library, "stock",
                module -> ((ArrayNode) module.get("lists")).removeAll()))
                .isInstanceOf(IllegalStateException.class)
                .hasMessageContaining("module `stock` nothing to build a list of");
    }

    /** One element twice in a module is two things said of one list. */
    @Test
    void aListOfOneElementSaidTwiceIsRefused(@TempDir Path into) throws Exception {
        NativeCompiler.Library library = twoModules(into);

        assertThatThrownBy(() -> generatedAfter(into, library, "shop", module -> {
            ArrayNode lists = (ArrayNode) module.get("lists");
            lists.add(lists.get(0).deepCopy());
        }))
                .isInstanceOf(IllegalStateException.class)
                .hasMessageContaining("module `shop` two lists of");
    }
}
