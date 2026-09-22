package souther.nativecode;

import org.junit.jupiter.api.Test;
import souther.compiler.Compiler;
import souther.compiler.jvm.ClassFileImage;
import souther.compiler.meta.ModulePath;
import souther.compiler.observe.ObservedValue;
import souther.compiler.program.CheckedImplementation;
import souther.compiler.program.CheckedModule;
import souther.compiler.program.CheckedProgram;
import souther.compiler.types.ValueName;
import souther.nativecode.transport.ProgramWriter;

import java.io.IOException;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.Path;
import java.util.ArrayList;
import java.util.LinkedHashMap;
import java.util.List;
import java.util.Map;

import static org.assertj.core.api.Assertions.assertThat;

/**
 * A behavior this program reaches and no module of it declares, and a value of a declared type
 * handed across the two objects that meet at it.
 *
 * <p>The module holding it was built before and is read off the path, so it is not among the
 * modules this compile emits. What a call can reach is therefore wider than what is emitted, and a
 * document taking one list for both leaves a call reaching a name nothing in it says anything
 * about.
 *
 * <p>Both objects are built here and the linker is given the pair, which is the only way to put
 * what two builds agree on to anything. Over numbers and truths there is nothing to agree: a
 * number is a number in either object, and a string is a count of bytes and then that many bytes
 * for the same reason — the layout is stated in the crate both halves of this backend read, so no
 * name is left for a linker to resolve and no two builds can have worked one out differently. A value of a declared type says which type it is, and what
 * it says has to mean the same thing to a fork in the other object — so the pair is where an
 * identity one object worked out for itself stops working, and where one the linker settles is the
 * whole of the answer.
 */
class ABehaviorAnotherBuildImplementsTest {

    private static final String BUILT_BEFORE = """
            module lib.rates exposing ( Rate, Shape, Round, Square, spin, tally, twice, rounded,
                                        squared, shout )

            data Rate = Int

            data Round = { across: Int }
            data Square = { side: Int }
            data Shape = Round | Square

            // `rounded` first, so that this document meets `Round` before anything else and the
            // one below meets it second. Which declaration a document meets first is a fact about
            // the order its bodies happen to be written in, and that is exactly what an identity
            // must not be made of.
            behavior rounded : (across: Int) -> Shape
            let rounded (across) = Round { across = across }

            behavior squared : (side: Int) -> Shape
            let squared (side) = Square { side = side }

            behavior spin : (of: Int) -> Rate
            let spin (of) = Rate(of)

            behavior tally : (r: Rate) -> Int
            let tally (r) = r.value

            behavior twice : (a: Int) -> Int
            let twice (a) = a * 2

            behavior shout : (a: String) -> String
            let shout (a) = a ++ "!"
            """;

    private static final String REACHING_IT = """
            module app.uses
            import lib.rates ( Rate, spin, tally )

            behavior counted : (base: Int) -> Int
            let counted (base) = tally(spin(base))
            """;

    /**
     * A value built in the other object, and a fork here on what it is.
     *
     * <p>Which is the whole of what this issue was about. The value is made where {@code rounded}
     * and {@code squared} are, in an object built from a document that never saw this one; the arms
     * are here, and what they compare is what the value carries.
     */
    private static final String FORKING_ON_IT = """
            module app.forks exposing ( sized )
            import lib.rates ( Shape, Round, Square, rounded, squared )

            behavior sized : (n: Int, round: Bool) -> Int
            let sized (n, round) = {
                let shape: Shape = if round then rounded(n) else squared(n)
                match shape with
                    | Round as r -> r.across * 2
                    | Square as q -> q.side * 4
            }
            """;

    /** What the setup rests on, asserted rather than assumed. */
    @Test
    void theModuleHoldingItIsNotOneThisCompileEmits() {
        CheckedProgram program = compiled(REACHING_IT);

        assertThat(program.modules().stream().map(CheckedModule::name).toList())
                .containsExactly("app.uses");
        assertThat(program.behavior(new ValueName.Behavior("lib.rates", "spin")).implementation())
                .isInstanceOf(CheckedImplementation.ImplementedElsewhere.class);
    }

    @Test
    void itCrossesWithTheSignatureACallerReachesItBy() {
        String written = ProgramWriter.written(compiled(REACHING_IT));

        assertThat(written).contains("\"module\":\"lib.rates\",\"name\":\"spin\",\"is\":\"elsewhere\"");
        assertThat(written).contains("\"module\":\"lib.rates\",\"name\":\"tally\",\"is\":\"elsewhere\"");
        // And what it takes and answers, which is a declaration no module here holds either.
        assertThat(written).contains("\"declared\":\"lib.rates.Rate\"");
    }

    /**
     * The declaration crosses saying who declared it, which is who defines its identity.
     *
     * <p>Not worked out on the far side from whether the module is one the document carries. It is
     * the checker's answer, and the far side reporting it back is what keeps the two halves from
     * each having a rule for it.
     */
    @Test
    void aDeclarationOfAModuleReadOffThePathSaysSo() {
        assertThat(ProgramWriter.written(compiled(REACHING_IT)))
                .contains("\"module\":\"lib.rates\",\"name\":\"Rate\",\"by\":\"onthepath\"");
        assertThat(ProgramWriter.written(builtBefore()))
                .contains("\"module\":\"lib.rates\",\"name\":\"Rate\",\"by\":\"amodule\"");
    }

    /**
     * A value of a declared type crosses out of the object it was built in.
     *
     * <p>What it says it is no longer depends on the document it was built from, so the signature
     * of a behavior another build answers may carry one.
     */
    @Test
    void aValueOfADeclaredTypeCrossesOutOfTheObjectItWasBuiltIn() throws Exception {
        assertThat(NativeCompiler.compile(compiled(REACHING_IT))).isNotEmpty();
    }

    /**
     * What the two documents disagree about, which is what gives the link below its teeth.
     *
     * <p>Each document brings the declarations its own bodies and signatures named, in the order
     * they were named, and the two orders are not the same one. An identity counted out of that
     * order is therefore a different number in each object for the one declaration — and a fork
     * comparing the two would answer by whichever the two happened to agree on, without either
     * object having anything to complain about.
     *
     * <p>Asserted rather than described, because it is the premise: were the two orders to come out
     * alike, the run below would pass over an identity that had never been put to anything.
     */
    @Test
    void theTwoDocumentsBringTheirDeclarationsInDifferentOrders() {
        String before = ProgramWriter.written(builtBefore());
        String forking = ProgramWriter.written(compiled(FORKING_ON_IT));

        assertThat(whereItStands(forking, "Round"))
                .as("Round stands in one place in %s and another in %s", before, forking)
                .isNotEqualTo(whereItStands(before, "Round"));
    }

    /**
     * One declaration is one name, defined by the object of the build that declared it and left
     * undefined by the object that names it.
     *
     * <p>Which is what makes the identity the linker's to settle rather than the two builds'. The
     * letter {@code nm} writes says which of the two each object is doing, and it is the same name
     * in both tables.
     */
    @Test
    void oneDeclarationIsOneNameTheLinkerResolves() throws Exception {
        Map<String, String> home = named(builtBefore());
        Map<String, String> naming = named(compiled(FORKING_ON_IT));
        String token = "souther$type$lib.rates$Round";

        // Defined and offered, whichever section the format puts a byte of read-only data in:
        // upper case is what `nm` writes for a name the table offers, and `U` for one it wants.
        assertThat(home.get(token))
                .as("what the declaring module's object carries: %s", home)
                .isNotNull()
                .isNotEqualTo("U")
                .matches("[A-Z]");
        assertThat(naming)
                .as("what the object naming it carries: %s", naming)
                .containsEntry(token, "U");
    }

    /**
     * A value built in one object, forked on in another, linked and run.
     *
     * <p>Both arms, because one of them is answered by a comparison that held and the other by one
     * that did not. A run that only ever took the first arm would be green over a fork that
     * answered the same way whatever it was handed.
     */
    @Test
    void aValueBuiltInOneObjectIsForkedOnInAnother() throws Exception {
        CheckedProgram program = compiled(FORKING_ON_IT);
        byte[] before = NativeCompiler.compile(builtBefore());

        try (Running running = Running.of(program, List.of(before))) {
            CheckedModule module = program.modules().getFirst();
            var sized = module.behavior(new ValueName.Behavior("app.forks", "sized"));

            assertThat(running.answering(module, sized,
                    List.of(new ObservedValue.Integer(5), new ObservedValue.Bool(true))))
                    .isEqualTo(new ObservedValue.Integer(10));
            assertThat(running.answering(module, sized,
                    List.of(new ObservedValue.Integer(5), new ObservedValue.Bool(false))))
                    .isEqualTo(new ObservedValue.Integer(20));
        }
    }

    /**
     * What crosses over numbers alone crosses the same way. The object names the behavior and
     * defines nothing for it, so what answers it is settled by whoever links the two objects.
     */
    @Test
    void aBehaviorAnotherBuildImplementsOverNumbersAloneIsReached() throws Exception {
        CheckedProgram program = compiled("""
                module app.plainly exposing ( fourTimes )
                import lib.rates ( twice )

                behavior fourTimes : (base: Int) -> Int
                let fourTimes (base) = twice(twice(base))
                """);
        byte[] before = NativeCompiler.compile(builtBefore());

        assertThat(ProgramWriter.written(program))
                .contains("\"module\":\"lib.rates\",\"name\":\"twice\",\"is\":\"elsewhere\"");
        try (Running running = Running.of(program, List.of(before))) {
            CheckedModule module = program.modules().getFirst();
            var fourTimes = module.behavior(new ValueName.Behavior("app.plainly", "fourTimes"));

            assertThat(running.answering(module, fourTimes,
                    List.of(new ObservedValue.Integer(3))))
                    .isEqualTo(new ObservedValue.Integer(12));
        }
    }

    /**
     * Text made in one object and joined and compared in another.
     *
     * <p>A string is an address and what is at it is made where the run that made it was, so the
     * pair is where a representation one object invented for itself would come apart. Both
     * directions are put to it: what the other object answered is joined here, and what this
     * object built is compared against what came back.
     */
    @Test
    void textCrossesOutOfTheObjectItWasMadeIn() throws Exception {
        CheckedProgram program = compiled("""
                module app.says exposing ( twiceOver, agrees )
                import lib.rates ( shout )

                behavior twiceOver : (a: String) -> String
                let twiceOver (a) = shout(shout(a))

                behavior agrees : (a: String) -> Bool
                let agrees (a) = shout(a) == (a ++ "!")
                """);
        byte[] before = NativeCompiler.compile(builtBefore());

        try (Running running = Running.of(program, List.of(before))) {
            CheckedModule module = program.modules().getFirst();
            var twiceOver = module.behavior(new ValueName.Behavior("app.says", "twiceOver"));
            var agrees = module.behavior(new ValueName.Behavior("app.says", "agrees"));

            assertThat(running.answering(module, twiceOver,
                    List.of(new ObservedValue.Text("hi"))))
                    .isEqualTo(new ObservedValue.Text("hi!!"));
            assertThat(running.answering(module, agrees,
                    List.of(new ObservedValue.Text("hi"))))
                    .isEqualTo(new ObservedValue.Bool(true));
        }
    }

    /**
     * Where a declaration stands among the ones the document brought.
     *
     * <p>Read off the document rather than out of the lowering, because what is being asked about
     * is what the two documents say — and the lowering no longer counts them at all.
     */
    private static int whereItStands(String written, String name) {
        String declarations = written.substring(
                written.indexOf("\"declarations\":["),
                written.indexOf("],\"behaviors\":"));
        int at = declarations.indexOf("\"name\":\"" + name + "\"");
        assertThat(at).as("%s is among %s", name, declarations).isNotNegative();
        return declarations.substring(0, at).split("\\{\"module\":", -1).length - 2;
    }

    /**
     * What the object says about each name it carries, by the letter {@code nm} writes for it.
     *
     * <p>The leading underscore Mach-O writes is taken off again, so what is asked about here is
     * the name the compiler gave.
     */
    private static Map<String, String> named(CheckedProgram program)
            throws IOException, InterruptedException {
        Path into = Files.createTempDirectory("souther-native-table");
        try {
            Path object = into.resolve("program.o");
            Files.write(object, NativeCompiler.compile(program));

            Process nm = new ProcessBuilder("nm", object.toString())
                    .redirectErrorStream(true)
                    .start();
            String said = new String(nm.getInputStream().readAllBytes(), StandardCharsets.UTF_8);
            assertThat(nm.waitFor()).as("nm said: %s", said).isZero();

            Map<String, String> table = new LinkedHashMap<>();
            for (String line : said.split("\n")) {
                String[] parts = line.strip().split("\\s+");
                if (parts.length < 2) {
                    continue;
                }
                String name = parts[parts.length - 1];
                String letter = parts[parts.length - 2];
                table.put(name.startsWith("_") ? name.substring(1) : name, letter);
            }
            return table;
        } finally {
            List<Path> held = new ArrayList<>();
            try (var walked = Files.walk(into)) {
                walked.forEach(held::add);
            }
            held.sort((a, b) -> b.getNameCount() - a.getNameCount());
            for (Path path : held) {
                Files.deleteIfExists(path);
            }
        }
    }

    private static CheckedProgram compiled(String source) {
        return CheckedProgram.of(List.of(source), path());
    }

    /**
     * The module that was built before, as the build that built it checked it.
     *
     * <p>Off the path and not on it: what it is to the other compile is a dependency, and what it
     * is to its own is the program being emitted. A compile handed both would be checking a module
     * against a build of itself.
     */
    private static CheckedProgram builtBefore() {
        return CheckedProgram.of(List.of(BUILT_BEFORE));
    }

    private static ModulePath path() {
        Map<String, ClassFileImage> published = Compiler.compile(BUILT_BEFORE);
        return ModulePath.of(published);
    }
}
