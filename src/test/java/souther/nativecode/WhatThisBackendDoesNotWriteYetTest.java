package souther.nativecode;

import org.junit.jupiter.api.Test;
import souther.compiler.program.CheckedProgram;
import souther.nativecode.transport.ProgramWriter;

import java.io.ByteArrayOutputStream;
import java.io.OutputStream;
import java.io.PrintStream;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.Path;
import java.util.List;

import static org.assertj.core.api.Assertions.assertThat;
import static org.assertj.core.api.Assertions.assertThatThrownBy;

/**
 * A program the language admits that this backend does not write yet, followed all the way out.
 *
 * <p>Three answers are possible at the end of this path and only one of them is right: the program
 * is refused by the language, this backend has not got round to it, or the command was wrong.
 * Which one a reader is told decides whether they go and change their program. So the test is the
 * whole way through rather than at any one of the places the answer could be lost.
 */
class WhatThisBackendDoesNotWriteYetTest {

    private static final String OVER_A_DECIMAL = """
            module calculation

            behavior widen : (a: Decimal) -> Decimal

            let widen (a) = a
            """;

    /**
     * The type crosses. What a primitive is called is the language's and whether there is a
     * representation for it is the driver's, so a writer holding its own list of what the driver
     * supports would be a second copy of an answer that lives over there.
     *
     * <p>The operator half of this is checked where a document can be written by hand
     * (`native/crates/compiler/tests/refusals.rs`): every operator a program can currently get
     * past this writer has a lowering, so there is no program to write here that would show it.
     * That every member of either vocabulary is spelt the same way on both sides is a different
     * question, and {@link souther.nativecode.transport.ProgramWriter#vocabularies} is what the two
     * halves meet at for it.
     */
    @Test
    void aTypeWithNoRepresentationStillCrosses() {
        String written = ProgramWriter.written(CheckedProgram.of(List.of(OVER_A_DECIMAL)));

        assertThat(written).contains("\"prim\":\"DECIMAL\"");
    }

    /**
     * A set crosses whole: the program is read, and what is refused is laying one out, which
     * nothing here does yet. How a set holds its members waits on the language saying how every
     * carrier orders and spells them.
     */
    @Test
    void anAnswerThatIsASetIsReadAndNotLaidOut() {
        assertThatThrownBy(() -> NativeCompiler.compile(CheckedProgram.of(List.of("""
                module listed exposing ( many )

                behavior many : (n: Int) -> Set<Int>
                """))))
                .isInstanceOf(NotLowered.class)
                .hasMessageContaining("Set");
    }

    /**
     * The checker lets a set of a case stand where a set of its sum is answered, and this backend
     * lays out no set. So the program is not lowered — and it is not the two halves disagreeing,
     * which is what it would be read as if this side answered the checker's question about a
     * collection without the checker's rules.
     */
    @Test
    void aSetAnsweredCovariantlyIsNotLoweredRatherThanADisagreement() {
        assertThatThrownBy(() -> NativeCompiler.compile(CheckedProgram.of(List.of("""
                module demo exposing ( f, Box, A, B, S )

                data A = { v: Int }
                data B = { v: Int }
                data S = A | B

                data Box = { xs: Set<A> }

                behavior f : (b: Box) -> Set<S>
                let f (b) = b.xs
                """))))
                .isInstanceOf(NotLowered.class)
                .hasMessageContaining("Set");
    }

    /**
     * A fold accumulating a map is rewritten by the checker's compiler into a walk that builds the
     * map, which crosses as the operations it is. No map is laid out here, so the walk is not
     * lowered, and that is what is said: not the two halves disagreeing about an operation one of
     * them could not read.
     */
    @Test
    void aWalkBuildingAMapIsReadAndNotLowered() {
        assertThatThrownBy(() -> NativeCompiler.compile(CheckedProgram.of(List.of("""
                module grouping exposing ( groups )

                behavior groups : (a: Int) -> Int
                let groups (a) = Map.size(List.groupBy((x) -> x > a, [1, 2, 3]))
                """))))
                .isInstanceOf(NotLowered.class)
                .hasMessageStartingWith("the operation Map.$");
    }

    /**
     * A fold seeded with a value holding {@code []} that the checker's compiler does not rewrite
     * hands its helper a seed and a step typed at that {@code []}, narrower than what the fold
     * settles and with nothing saying it stands wider (souther-lang/souther#1958). Refused as not
     * lowered, naming that, until the tree says it.
     */
    @Test
    void aFoldSeededWithAnEmptyListItDoesNotGrowIsNotLoweredYet() {
        assertThatThrownBy(() -> NativeCompiler.compile(CheckedProgram.of(List.of("""
                module dropping exposing ( rest )

                behavior rest : (a: Int) -> Int
                let rest (a) = List.length(List.drop(a, [1, 2, 3]))
                """))))
                .isInstanceOf(NotLowered.class)
                .hasMessageContaining("souther-lang/souther#1958");
    }

    /**
     * A fold over an empty list literal that is not rewritten into a walk hands its helper a
     * function over what has no value, which it never applies. The checker's backend hands
     * {@code Fn.NEVER} in its place; a copy here would have to take a function over a type nothing
     * lays out, so it is refused as not lowered, and nothing of the function is.
     */
    @Test
    void aFunctionAHelperNeverAppliesIsNotLoweredYet() {
        assertThatThrownBy(() -> NativeCompiler.compile(CheckedProgram.of(List.of("""
                module folding exposing ( kept )

                behavior kept : (a: Int) -> Int
                let kept (a) = List.fold((acc, x) -> acc, a, [])
                """))))
                .isInstanceOf(NotLowered.class)
                .hasMessageContaining("it never applies");
    }

    /**
     * A value only passing through is not written, so a behavior handing one back compiles; what
     * is refused is the boundary that would have to write a {@code Decimal} out, which is where
     * its canonical form would be decided.
     */
    @Test
    void anAnswerWithADecimalFieldIsRefusedWhereItWouldBeWrittenOut() {
        assertThatThrownBy(() -> NativeCompiler.compile(CheckedProgram.of(List.of("""
                module priced exposing ( same, Priced )

                data Priced = { amount: Decimal }

                behavior same : (p: Priced) -> Priced
                let same (p) = p
                """))))
                .isInstanceOf(NotLowered.class)
                .hasMessageContaining("Decimal");
    }

    /**
     * A row's entry calls what computes each of its inputs, so an input whose operand has no
     * expression here is a row the object cannot run.
     *
     * <p>Refused rather than left out. An object missing an entry would still link and still
     * answer every row it did carry, so what a check of the rows compared would shrink by however
     * many rows had values like this one — and it would go on being green over the ones that were
     * left.
     *
     * <p>A date is the value here. What refuses it is the definition computing the input, which
     * the module holds and which is written like any other of its helpers.
     */
    @Test
    void aRowStatingAValueWithNoExpressionToMakeItIsRefusedRatherThanLeftOut() {
        CheckedProgram program = CheckedProgram.of(List.of("""
                module owing

                data Due = { on: Date, label: String }

                behavior labelled : (due: Due) -> String
                let labelled (due) = due.label

                example labelled
                    | "a date" : (Due { on = Date("2026-07-25"), label = "rent" }) -> "rent"
                """));

        assertThatThrownBy(() -> ProgramWriter.written(program))
                .isInstanceOf(NotLowered.class)
                .hasMessageContaining("a temporal literal");
    }

    /**
     * A value of a type the language declares, which is at home in no build's object.
     *
     * <p>A declared type's values are tagged by the address of a byte one object defines, and the
     * object that defines it is the one built from the module that declared the type. The language
     * declares {@code RoundingMode} and its cases in its own namespace, in no module of any
     * compilation, so there is no such object — an implementation of one is shipped by hand or
     * generated, and which of the two is a question this backend has not answered.
     *
     * <p>Followed from the writer to the driver, which is what makes this the third answer about
     * who declared a type rather than the two a program of modules alone can produce. What the
     * driver answers is what says which of the three it read: a declaration of a module here would
     * have had its token defined and one off the path named, and either of those compiles.
     */
    @Test
    void aValueOfATypeTheLanguageDeclaresIsAtHomeInNoObject() {
        CheckedProgram program = CheckedProgram.of(List.of("""
                module rounding

                behavior of : (a: Int) -> Int
                let of (a) = {
                    let mode = HALF_UP
                    a
                }
                """));

        assertThat(ProgramWriter.written(program)).contains(
                "\"module\":\"souther.decimal\",\"name\":\"HALF_UP\",\"by\":\"thelanguage\"");
        assertThatThrownBy(() -> NativeCompiler.compile(program))
                .isInstanceOf(NotLowered.class)
                .hasMessageContaining("souther.decimal.HALF_UP");
    }

    /**
     * A value of a newtype compared with a bare literal, which is a comparison of what it wraps.
     *
     * <p>Both orders, and the order is the point. The checker reads the pair as values of the
     * newtype, for this operator only, and says so on the node whichever side the literal is on:
     * an `Int` on one side and an address on the other is what the operands are, and what they are
     * read as is the newtype. Nothing here takes a literal as a newtype yet, so both orders are
     * refused, and for that reason.
     */
    @Test
    void aNewtypeComparedWithABareLiteralIsNotComparedByWhereItIs() {
        for (String body : List.of("0 == a", "a == 0", "100 <= a", "a >= 100")) {
            CheckedProgram program = CheckedProgram.of(List.of("""
                    module comparing

                    data Amount = Int

                    behavior asked : (a: Amount) -> Bool
                    let asked (a) = %s
                    """.formatted(body)));
            assertThat(ProgramWriter.written(program))
                    .as("`%s`", body)
                    .contains("\"reading\":{\"is\":\"in\",\"type\":{\"declared\":\"comparing.Amount\"}}");
            assertThatThrownBy(() -> NativeCompiler.compile(program))
                    .as("`%s`", body)
                    .isInstanceOf(NotLowered.class)
                    .hasMessageContaining("read as comparing.Amount");
        }
    }

    /**
     * A sum compared with one of its cases, which is two declared types that are not one type.
     *
     * <p>A case value is a value of its sum, so this is as legitimate as comparing two values of
     * one type — and it arrives with a different declaration named on each side. What it comes to
     * is which case the value is, which is not written here either.
     */
    @Test
    void aSumComparedWithOneOfItsCasesIsNotComparedByWhereItIs() {
        assertThatThrownBy(() -> NativeCompiler.compile(CheckedProgram.of(List.of("""
                module staging

                data Prospecting
                data Won
                data Stage = Prospecting | Won

                behavior done : (s: Stage) -> Bool
                let done (s) = s == Won
                """))))
                .isInstanceOf(NotLowered.class)
                .hasMessageContaining("staging.Stage");
    }

    @Test
    void theDriverSaysItIsOneThisBackendHasNotGotRoundTo() {
        assertThatThrownBy(() -> NativeCompiler.compile(CheckedProgram.of(List.of(OVER_A_DECIMAL))))
                .isInstanceOf(NotLowered.class)
                .hasMessageContaining("Decimal");
    }

    @Test
    void theCommandLineSaysTheBackendIsBehindAndNotThatTheCommandWasWrong() throws Exception {
        Path source = Files.createTempDirectory("souther-native-test").resolve("calculation.sou");
        Files.writeString(source, OVER_A_DECIMAL, StandardCharsets.UTF_8);
        ByteArrayOutputStream problems = new ByteArrayOutputStream();

        int ended = Main.run(
                new String[]{"-o", source.resolveSibling("out.o").toString(), source.toString()},
                new PrintStream(OutputStream.nullOutputStream(), true, StandardCharsets.UTF_8),
                new PrintStream(problems, true, StandardCharsets.UTF_8));

        assertThat(ended).isEqualTo(1);
        assertThat(problems.toString(StandardCharsets.UTF_8))
                .contains("this backend does not write that yet");
    }
}
