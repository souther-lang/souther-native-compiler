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
     * A list crosses whole: the program is read, and what is refused is laying one out, which
     * nothing here does yet. Its external form waits on the language saying how every carrier
     * orders and spells what a collection holds.
     */
    @Test
    void anAnswerThatIsAListIsReadAndNotLaidOut() {
        assertThatThrownBy(() -> NativeCompiler.compile(CheckedProgram.of(List.of("""
                module listed exposing ( many )

                behavior many : (n: Int) -> List<Int>
                """))))
                .isInstanceOf(NotLowered.class)
                .hasMessageContaining("List");
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
     * A construction runs the type's clauses and stops at the first that does not hold. Nothing
     * here runs one, and building the value anyway would make the type's invariant true of what
     * this emits by leaving it out.
     */
    @Test
    void aTypeThatSaysWhatItsValuesOweIsOneNothingIsBuiltOfYet() {
        assertThatThrownBy(() -> NativeCompiler.compile(CheckedProgram.of(List.of("""
                module owing

                data Amount = { value: Int }
                    invariant value >= 0

                behavior of : (a: Int) -> Int
                let of (a) = {
                    let held = Amount { value = a }
                    held.value
                }
                """))))
                .isInstanceOf(NotLowered.class)
                .hasMessageContaining("owing.Amount");
    }

    /**
     * A row states its values and the object carries an entry that runs them, so a value with no
     * expression to make it is a row the object cannot run.
     *
     * <p>Refused rather than left out. An object missing an entry would still link and still
     * answer every row it did carry, so what a check of the rows compared would shrink by however
     * many rows had values like this one — and it would go on being green over the ones that were
     * left.
     */
    @Test
    void aRowStatingAValueWithNoExpressionToMakeItIsRefusedRatherThanLeftOut() {
        CheckedProgram program = CheckedProgram.of(List.of("""
                module owing

                data Amount = { value: Int }

                behavior tally : (a: Amount) -> Int
                let tally (a) = a.value

                example tally
                    | "a value of it" : (Amount { value = 7 }) -> 7
                """));

        assertThatThrownBy(() -> ProgramWriter.written(program))
                .isInstanceOf(NotLowered.class)
                .hasMessageContaining("a row stating")
                .hasMessageContaining("owing.Amount");
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
     * Two values of a declared type compared, which is what they are made of and not where they
     * are.
     *
     * <p>Souther's {@code ==} over a value of a declared type is its fields compared one by one, so
     * two built separately out of the same field values are equal. A value here is held as the
     * address of what it is made of, and a comparison of the two addresses answers a different
     * question — one whose answer is false for exactly the pair the language calls equal.
     *
     * <p>So the refusal is the point. An object that compared the addresses would link, run, and
     * answer, and nothing downstream would have anything to notice.
     */
    @Test
    void twoValuesOfADeclaredTypeAreNotComparedByWhereTheyAre() {
        assertThatThrownBy(() -> NativeCompiler.compile(CheckedProgram.of(List.of("""
                module comparing

                data Employee = { id: Int }

                behavior same : (a: Employee, b: Employee) -> Bool
                let same (a, b) = a == b
                """))))
                .isInstanceOf(NotLowered.class)
                .hasMessageContaining("comparing.Employee");
    }

    /**
     * A value of a newtype compared with a bare literal, which is a comparison of what it wraps.
     *
     * <p>Both orders, and the order is the point. The language lets a bare literal take the
     * newtype of the operand it is compared with, so `0 == amount` crosses with an `Int` on the
     * left and a declared type on the right — and a lowering that read the left operand alone
     * would compare two `Int`s, one of which is an address. It would then refuse `amount == 0`,
     * which means the same thing, and which of the two a program got would be the order its author
     * wrote them in.
     */
    @Test
    void aNewtypeComparedWithABareLiteralIsNotComparedByWhereItIs() {
        for (String body : List.of("0 == a", "a == 0", "100 <= a", "a >= 100")) {
            assertThatThrownBy(() -> NativeCompiler.compile(CheckedProgram.of(List.of("""
                    module comparing

                    data Amount = Int

                    behavior asked : (a: Amount) -> Bool
                    let asked (a) = %s
                    """.formatted(body)))))
                    .as("`%s`", body)
                    .isInstanceOf(NotLowered.class)
                    .hasMessageContaining("comparing.Amount");
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

    /**
     * The same of an optional, which is equal where both hold nothing and where both hold values
     * that compare equal.
     *
     * <p>Written out rather than taken as covered by the declared type above: an optional holding
     * nothing is a null pointer here, so two of those do compare equal by address, and a check that
     * only ever compared absent ones would be green over the half of the question that happens to
     * agree.
     *
     * <p>Reached through a field because that is where the language admits one: an optional is
     * written on a data field or inferred, and never named on a behavior's own signature.
     */
    @Test
    void twoOptionalsAreNotComparedByWhereTheyAre() {
        assertThatThrownBy(() -> NativeCompiler.compile(CheckedProgram.of(List.of("""
                module comparing

                data Employee = { manager: Int? }

                behavior same : (a: Employee, b: Employee) -> Bool
                let same (a, b) = a.manager == b.manager
                """))))
                .isInstanceOf(NotLowered.class)
                .hasMessageContaining("an Option of Int");
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
