package souther.nativecode;

import souther.compiler.abort.AbortKind;
import souther.compiler.observe.ObservedValue;
import souther.compiler.observe.StoodIn;
import souther.compiler.program.CheckedBehavior;
import souther.compiler.program.CheckedImplementation;
import souther.compiler.program.CheckedModule;
import souther.compiler.program.CheckedProgram;
import souther.compiler.program.CheckedSignature;
import souther.compiler.program.Publication;
import souther.compiler.program.StandsIn;
import souther.compiler.types.Type;
import souther.compiler.types.ValueName;

import java.io.IOException;
import java.io.UncheckedIOException;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.Path;
import java.util.ArrayList;
import java.util.HashMap;
import java.util.LinkedHashMap;
import java.util.List;
import java.util.Map;
import java.util.StringJoiner;

/**
 * A behavior of this project's, linked and callable.
 *
 * <p>Everything this backend claims is claimed about an object file, so a test that did not link
 * one and run it would be checking this project against itself. This is what runs it: the system
 * linker resolves the symbol, the process answers, and what comes back is read as a Souther value
 * for the row to be asked about.
 *
 * <p>Two ways in, because there are two questions. A behavior the module publishes is reached by
 * its own symbol and handed values on the command line. A row is reached through the entry the
 * object carries for it, whatever the module says about the behavior's name — the values are in
 * the object and running a row is not reaching the behavior.
 *
 * <p>The spelling of a symbol is written here a second time, which is the one place in this project
 * it is. The compiler hands Cranelift the name from the {@code abi} crate and never sees it again;
 * a C declaration is the only other thing that has to say it, and it has to say it in C.
 */
final class Running implements AutoCloseable {

    private final CheckedProgram program;
    private final Path into;
    private final Path object;
    private final List<Path> alongside;
    private final Map<String, Path> linked = new HashMap<>();

    private Running(CheckedProgram program, Path into, Path object, List<Path> alongside) {
        this.program = program;
        this.into = into;
        this.object = object;
        this.alongside = alongside;
    }

    /** The program compiled to one object, with nothing linked yet. */
    static Running of(CheckedProgram program) throws IOException, InterruptedException {
        return of(program, List.of());
    }

    /**
     * The same, with objects another build wrote linked in beside it.
     *
     * <p>Which is the only way to put what two objects agree on to anything: a behavior this
     * program names and does not define is answered by whoever links it, and a C stand-in
     * answering it is this test deciding what the dependency does. An object another Souther build
     * emitted decides it the way a build does, and what the two objects then have to agree about —
     * a symbol, a signature, what a value of a declared type says it is — is agreed through the
     * linker or not at all.
     */
    static Running of(CheckedProgram program, List<byte[]> alongside)
            throws IOException, InterruptedException {
        Path into = Files.createTempDirectory("souther-native-running");
        Path object = into.resolve("program.o");
        Files.write(object, NativeCompiler.compile(program));
        List<Path> written = new ArrayList<>();
        for (byte[] built : alongside) {
            Path beside = into.resolve("alongside." + written.size() + ".o");
            Files.write(beside, built);
            written.add(beside);
        }
        return new Running(program, into, object, List.copyOf(written));
    }

    /**
     * What the behavior answers when it is handed these values.
     *
     * <p>An abort is not an answer and does not come back as one: a caller expecting a row's stated
     * value throws rather than reads a {@link RunOutcome.Aborted} as though it were one.
     */
    ObservedValue answering(CheckedModule module, CheckedBehavior behavior,
                            List<ObservedValue> inputs) throws IOException, InterruptedException {
        return answering(module, behavior, inputs, List.of());
    }

    ObservedValue answering(CheckedModule module, CheckedBehavior behavior,
                            List<ObservedValue> inputs, List<StandsIn> standIns)
            throws IOException, InterruptedException {
        return switch (answeredOrEnded(module, behavior, inputs, standIns)) {
            case RunOutcome.Answered it -> it.value();
            case RunOutcome.Aborted it -> throw new AssertionError(
                    "the run ended with " + it.kind() + " rather than answering: "
                            + behavior.name() + " of " + module.name() + ", handed " + inputs);
        };
    }

    /**
     * What the behavior answered, or the reason the run ended without one.
     *
     * <p>For a caller whose question is which of the two happened, and — since issue #9 — which
     * {@link AbortKind} it was where the run ended. A caller asking it is expected to have a pair
     * of runs where one of each happens, since an ended run says nothing on its own about whether
     * the neighbour it is read beside answers instead by chance or by the check this exists for.
     */
    RunOutcome answeredOrEnded(CheckedModule module, CheckedBehavior behavior,
                               List<ObservedValue> inputs)
            throws IOException, InterruptedException {
        return answeredOrEnded(module, behavior, inputs, List.of());
    }

    RunOutcome answeredOrEnded(CheckedModule module, CheckedBehavior behavior,
                               List<ObservedValue> inputs, List<StandsIn> standIns)
            throws IOException, InterruptedException {
        published(module, behavior);
        CheckedSignature signature = behavior.signature();
        String reached = module.name() + "." + behavior.name().name();
        Path executable = linked(reached, "souther." + reached, signature.answers(),
                signature.takes(), standIns);
        return ran(executable, signature.answers(), inputs);
    }

    /** What the behavior answered when this one of its rows was run. */
    ObservedValue rowAnswering(CheckedModule module, CheckedBehavior behavior, int at,
                               List<StandsIn> standIns) throws IOException, InterruptedException {
        return switch (rowAnsweredOrEnded(module, behavior, at, standIns)) {
            case RunOutcome.Answered it -> it.value();
            case RunOutcome.Aborted it -> throw new AssertionError(
                    "row " + at + " of " + behavior.name() + " of " + module.name()
                            + " ended with " + it.kind() + " rather than answering");
        };
    }

    /**
     * What one of the behavior's rows answers, run through the entry the object carries for it.
     *
     * <p>Not the behavior handed the row's values: the values were written into the object when
     * the program crossed, and the entry is what the object offers to whoever runs its rows. Which
     * is what lets a row of a name the module keeps be run at all — a module's surface is what the
     * behavior's own symbol answers to, and a row is a different question.
     */
    RunOutcome rowAnsweredOrEnded(CheckedModule module, CheckedBehavior behavior,
                                  int at, List<StandsIn> standIns)
            throws IOException, InterruptedException {
        String named = module.name() + "." + behavior.name().name() + ".example." + at;
        String symbol = "souther." + module.name() + "." + behavior.name().name()
                + "$example$" + at;
        Path executable = linked(named, symbol, behavior.signature().answers(), List.of(),
                standIns);
        return ran(executable, behavior.signature().answers(), List.of());
    }

    /**
     * What the harness's own two lines say: a status on the first, and — only where it is
     * {@code ANSWERED} — the value on the second. A process that failed on its own account, before
     * it could write either line, is neither {@link RunOutcome} case: it is this harness's own
     * failure and not a Souther computation's, so it is thrown rather than folded into one of them.
     */
    private RunOutcome ran(Path executable, Type answers, List<ObservedValue> inputs)
            throws IOException, InterruptedException {
        List<String> command = new ArrayList<>();
        command.add(executable.toString());
        for (ObservedValue given : inputs) {
            command.add(written(given));
        }

        Process process = new ProcessBuilder(command)
                .redirectErrorStream(true)
                .start();
        String said = new String(process.getInputStream().readAllBytes(), StandardCharsets.UTF_8);
        if (process.waitFor() != 0) {
            throw new AssertionError("the harness itself failed rather than answering a status: "
                    + said);
        }
        // Not said.strip().lines(): a value written on the second line can itself be empty — an
        // empty string's hex is no digits at all, just the newline writeText always ends on — and
        // stripping the whole blob first collapses that trailing empty line away before lines()
        // ever sees it, reading ANSWERED "" as though nothing had been written at all.
        List<String> lines = said.lines().toList();
        if (lines.isEmpty()) {
            throw new AssertionError("the harness wrote no status: " + said);
        }
        int status = Integer.parseInt(lines.get(0).strip());
        if (status == ANSWERED) {
            if (lines.size() < 2) {
                throw new AssertionError("ANSWERED with no value on the line under it: " + said);
            }
            return new RunOutcome.Answered(read(answers, lines.get(1)));
        }
        return new RunOutcome.Aborted(abortKindOf(status));
    }

    /**
     * The one status a generated function's status answers with when the pointer it was handed
     * holds the value. Written here a second time for the reason the C declaration in
     * {@link #harnessFor} is — {@code souther_native_abi::ANSWERED} is what generated code and the
     * native runtime agree on, and a test harness is a third party to that agreement that has to
     * say it in C and, to read it back, in Java.
     */
    private static final int ANSWERED = 0;

    /**
     * The wire number a status this harness read that is not {@link #ANSWERED} maps back to, held
     * to {@code native_status} in {@code souther-native-driver} the way {@link #ANSWERED} is held
     * to {@code souther_native_abi}: written here a second time because a test reading a native
     * run's own answer is, like the harness itself, on the far side of that crate's own boundary.
     * No default arm, so a member {@link AbortKind} adds and this driver's own mapping answers for
     * stops this compiling rather than this test reading it as whichever member happened to sit at
     * that number last.
     */
    private static AbortKind abortKindOf(int status) {
        return switch (status) {
            case 1 -> AbortKind.INVARIANT_NOT_HELD;
            case 2 -> AbortKind.ENSURES_NOT_HELD;
            case 3 -> AbortKind.UNREACHABLE_REACHED;
            case 4 -> AbortKind.DIVISION_BY_ZERO;
            case 5 -> AbortKind.REQUIRED_FORM_HAS_NO_PLACE;
            case 6 -> AbortKind.INVALID_BOUNDS;
            default -> throw new AssertionError("a status this harness has no AbortKind for: "
                    + status);
        };
    }

    /**
     * That the module publishes the name about to be reached.
     *
     * <p>A name a module keeps is named by that module and nothing else, and the object says so:
     * the symbol is local to it. Asked here rather than found out from a link that came up short,
     * so that reaching one says what was reached for.
     */
    private static void published(CheckedModule module, CheckedBehavior behavior) {
        if (module.publicationOf(behavior.name()) != Publication.PUBLISHED) {
            throw new AssertionError("`" + module.name() + "` keeps " + behavior.name()
                    + ", so nothing outside the object reaches it; a row of it is run through the"
                    + " entry the object carries for that row");
        }
    }

    private Path linked(String named, String symbol, Type answers, List<Type> takes,
                        List<StandsIn> standIns) throws IOException {
        // What stands in for a dependency is part of what is linked, so two rows of one behavior
        // that state different stand-ins are two executables and not one reused. The key is what
        // was stated and not a number worked out from it: a number that collides hands back an
        // executable built for a different table, and the answers would look like the lowering's.
        String key = named + " " + standIns;
        Path already = linked.get(key);
        if (already != null) {
            return already;
        }

        // The file is named by a count because what the key holds is not a file name.
        String name = named + "." + linked.size();
        Path harness = into.resolve(name + ".c");
        Files.writeString(harness, harnessFor(symbol, answers, takes, standIns),
                StandardCharsets.UTF_8);
        Path executable = into.resolve(name);

        List<String> link = new ArrayList<>(List.of("cc", "-o", executable.toString(),
                harness.toString(), object.toString()));
        for (Path beside : alongside) {
            link.add(beside.toString());
        }
        link.add(RUNTIME.toString());
        Process cc = new ProcessBuilder(link)
                .redirectErrorStream(true)
                .start();
        String said = new String(cc.getInputStream().readAllBytes(), StandardCharsets.UTF_8);
        try {
            if (cc.waitFor() != 0) {
                throw new AssertionError("the link failed: " + said);
            }
        } catch (InterruptedException e) {
            Thread.currentThread().interrupt();
            throw new UncheckedIOException(new IOException("interrupted while linking"));
        }
        linked.put(key, executable);
        return executable;
    }

    /** What a program made here calls for room, linked in beside what this compiler emitted. */
    private static final Path RUNTIME =
            Path.of("native", "target", "debug", "libsouther_native_runtime.a");

    /**
     * A C program that reaches the one symbol and writes what it answered.
     *
     * <p>Written for what is being reached rather than dispatched at run time: the arity and the
     * widths are what a call is made of, and a harness that took them as data would be making the
     * call from something other than what is being called.
     *
     * <p>The call is bracketed, which is how a Souther value is freed: nothing frees one on its
     * own, and what the run made is dropped in a single go by the caller. Bracketed here even
     * where one call is all that happens, because a harness that never gave the room back would be
     * a harness the contract had never been put to.
     */
    private String harnessFor(String symbol, Type answers, List<Type> takes,
                              List<StandsIn> standIns) {
        List<String> taken = new ArrayList<>();
        List<String> given = new ArrayList<>();
        for (int at = 0; at < takes.size(); at++) {
            taken.add(cType(takes.get(at)));
            given.add(read(takes.get(at), at + 1));
        }
        // One more parameter than the Souther signature shows, the same as every generated
        // function: room the answer is written through, in place of a plain return.
        taken.add(cType(answers) + " *");
        given.add("&answered");

        // Every name the object left undefined, and not only the ones this row states. The object
        // is the whole program, so what it names is what the linker wants whichever behavior is
        // being run — a dependency this row says nothing about is still a symbol with nothing
        // under it.
        //
        // What each of them is called in C is counted out here, so that nothing downstream has a
        // Souther identity to make a C identifier from.
        StringJoiner supplied = new StringJoiner("\n");
        int counted = 0;
        for (Map.Entry<ValueName.Behavior, StandsIn> named : leftUndefined(standIns).entrySet()) {
            supplied.add(standingIn("standsIn" + counted++, named.getKey(), named.getValue()));
        }

        return """
                #include <inttypes.h>
                #include <stdint.h>
                #include <stdio.h>
                #include <stdlib.h>
                #include <string.h>

                %s%s

                extern uint32_t reached(%s) __asm__("%s%s");
                extern int64_t souther_mark(void);
                extern void souther_reset(int64_t);

                int main(int argc, char **argv) {
                    if (argc != %d) {
                        return 2;
                    }
                    int64_t mark = souther_mark();
                    %s answered;
                    uint32_t status = reached(%s);
                    printf("%%u\\n", status);
                    if (status == 0) {
                        %s
                    }
                    souther_reset(mark);
                    return 0;
                }
                """.formatted(
                textCrossesHere(answers, takes) ? TEXT_CROSSING : "",
                supplied.toString(),
                takenIn(taken),
                PREFIX, symbol,
                takes.size() + 1,
                cType(answers),
                String.join(", ", given),
                writing(answers));
    }

    /**
     * Every behavior the object names and defines nothing for, with what the row says it answers
     * where the row says anything.
     *
     * <p>The row says what the dependency answers, entry by entry, and this is that table as the
     * definition the linker was missing. Arguments it was not told about end the run rather than
     * answering something: a stand-in asked for what the row never stated would be this harness
     * deciding what the dependency does, which is the row's to say.
     *
     * <p>A dependency the row is silent about still has to be defined for the object to link, and
     * what it answers is nothing the row stated — so it ends the run, the same as an argument the
     * table was not told about.
     */
    private Map<ValueName.Behavior, StandsIn> leftUndefined(List<StandsIn> standIns) {
        Map<ValueName.Behavior, StandsIn> supplied = new LinkedHashMap<>();
        for (CheckedModule module : program.modules()) {
            for (CheckedBehavior behavior : module.behaviors()) {
                if (behavior.implementation() instanceof CheckedImplementation.Injected) {
                    supplied.put(behavior.name(), null);
                }
            }
        }
        for (StandsIn standsIn : standIns) {
            supplied.put(standsIn.dependency(), standsIn);
        }
        return supplied;
    }

    /**
     * A C definition for one behavior the object names and does not define.
     *
     * <p>What it is called in C is handed in, and it is a physical name that means nothing. The
     * whole of what this definition is for is in the {@code __asm__} string, which carries a
     * Souther identity entire — a module and a name. A C identifier made here out of part of one
     * would be that identity written twice, once whole and once with the module dropped, and two
     * modules declaring a dependency of one name would collide in a translation unit that holds
     * every undefined name the object left. Which is every one of them, whatever this row states.
     */
    private String standingIn(String reached, ValueName.Behavior dependency, StandsIn standsIn) {
        CheckedSignature signature = program.behavior(dependency).signature();
        List<Type> takes = signature.takes();
        List<String> taken = new ArrayList<>();
        for (int at = 0; at < takes.size(); at++) {
            taken.add(cType(takes.get(at)) + " a" + at);
        }
        // One more than the Souther signature shows, the same as every generated function: this
        // stands in for a symbol a generated object calls through call_reached, which hands every
        // callee room for the answer and reads a status back rather than trusting a plain return.
        taken.add(cType(signature.answers()) + " *out");
        String parameters = takenIn(taken);

        StringBuilder answering = new StringBuilder();
        String otherwise = "    exit(3);\n";
        if (standsIn != null) {
            StoodIn stated = standsIn.stated();
            for (StoodIn.Entry entry : stated.entries()) {
                List<String> asked = new ArrayList<>();
                for (int at = 0; at < entry.arguments().size(); at++) {
                    asked.add("a" + at + " == " + asC(entry.arguments().get(at)));
                }
                answering.append("    if (%s) { *out = %s; return 0; }\n"
                        .formatted(everyOneOf(asked), asC(entry.answer())));
            }
            otherwise = switch (stated.otherwise()) {
                case StoodIn.Otherwise.Answer it -> "    *out = " + asC(it.value()) + ";\n    return 0;\n";
                case StoodIn.Otherwise.NothingStated it -> "    exit(3);\n";
            };
        }

        String symbol = PREFIX + "souther." + dependency.module() + "." + dependency.name();
        return "uint32_t %s(%s) __asm__(\"%s\");\nuint32_t %s(%s) {\n%s%s}"
                .formatted(reached, parameters, symbol, reached, parameters, answering, otherwise);
    }

    /**
     * A condition that holds where every one of these does.
     *
     * <p>Where there are none it is what holds anyway. A behavior that takes nothing is asked one
     * way and no other, so an entry stating no arguments is the entry for that call — and joining
     * nothing without saying what joining nothing comes to leaves an `if ()` no compiler reads.
     */
    private static String everyOneOf(List<String> asked) {
        if (asked.isEmpty()) {
            return "1";
        }
        return String.join(" && ", asked);
    }

    /**
     * What a C declaration takes, written the way C spells taking nothing.
     *
     * <p>Empty parentheses say a function whose parameters are unspecified rather than one that
     * takes none, which is a different declaration and not the one meant here.
     */
    private static String takenIn(List<String> taken) {
        if (taken.isEmpty()) {
            return "void";
        }
        return String.join(", ", taken);
    }

    /**
     * What the linker on this platform calls a symbol the object names.
     *
     * <p>Mach-O writes an underscore before every one and ELF writes none. The object carries
     * whichever its format takes, so what needs saying here is only what a C declaration has to be
     * written with to reach it.
     */
    private static final String PREFIX =
            System.getProperty("os.name", "").toLowerCase().contains("mac") ? "_" : "";

    /** Whether this harness has a string to make or to write out. */
    private static boolean textCrossesHere(Type answers, List<Type> takes) {
        if (prim(answers) == Type.Prim.STRING) {
            return true;
        }
        for (Type taken : takes) {
            if (prim(taken) == Type.Prim.STRING) {
                return true;
            }
        }
        return false;
    }

    /**
     * What this harness makes a string with and writes one back as.
     *
     * <p>Hex both ways, which is not for anyone to read. A string is bytes and a Souther one may
     * hold a newline or a nought; handed over as itself it would be cut short by the first nought
     * and read back wrongly at the first newline, and neither would look like a string being
     * mishandled — it would look like the program having answered something else.
     *
     * <p>The layout is nowhere here. A string is made and taken apart through the runtime, which
     * is the one place besides the {@code abi} crate that says what one is made of.
     */
    private static final String TEXT_CROSSING = """
            extern const uint8_t *souther_string_of_utf8(const uint8_t *, int64_t);
            extern int64_t souther_string_length(const uint8_t *);
            extern const uint8_t *souther_string_bytes(const uint8_t *);

            static const uint8_t *readText(const char *hex) {
                size_t length = strlen(hex) / 2;
                uint8_t *bytes = malloc(length + 1);
                for (size_t at = 0; at < length; at++) {
                    unsigned byte;
                    sscanf(hex + 2 * at, "%2x", &byte);
                    bytes[at] = (uint8_t) byte;
                }
                const uint8_t *held = souther_string_of_utf8(bytes, (int64_t) length);
                free(bytes);
                return held;
            }

            static void writeText(const uint8_t *held) {
                int64_t length = souther_string_length(held);
                const uint8_t *bytes = souther_string_bytes(held);
                for (int64_t at = 0; at < length; at++) {
                    printf("%02x", bytes[at]);
                }
                printf("\\n");
            }

            """;

    /** What the harness does with what came back, which is not one statement for every type. */
    private static String writing(Type answers) {
        return switch (prim(answers)) {
            case INT -> "printf(\"%\" PRId64 \"\\n\", answered);";
            case BOOL -> "printf(\"%d\\n\", answered);";
            case STRING -> "writeText(answered);";
            default -> throw new AssertionError("no harness writes a " + answers + " yet");
        };
    }

    private static String cType(Type type) {
        return switch (prim(type)) {
            case INT -> "int64_t";
            case BOOL -> "int8_t";
            case STRING -> "const uint8_t *";
            default -> throw new AssertionError("no harness writes a " + type + " yet");
        };
    }

    private static String read(Type type, int at) {
        return switch (prim(type)) {
            case INT -> "strtoll(argv[" + at + "], NULL, 10)";
            case BOOL -> "(int8_t) (strtoll(argv[" + at + "], NULL, 10) != 0)";
            case STRING -> "readText(argv[" + at + "])";
            default -> throw new AssertionError("no harness reads a " + type + " yet");
        };
    }

    /** A value as the command line carries it, which is where a run is handed what it takes. */
    private static String written(ObservedValue given) {
        return switch (given) {
            case ObservedValue.Integer it -> Long.toString(it.value());
            case ObservedValue.Bool it -> it.value() ? "1" : "0";
            case ObservedValue.Text it -> hex(it.value().getBytes(StandardCharsets.UTF_8));
            default -> throw new AssertionError("no harness hands over a " + given + " yet");
        };
    }

    /**
     * The same value as a C expression, which is a different question.
     *
     * <p>A stand-in is a table written into the harness rather than values handed to a process, so
     * what it takes is what C spells and not what a command line carries. The two agree for a
     * number and for a truth, and a string is where they stop agreeing: it is made rather than
     * written, and two of them are compared through the runtime rather than with {@code ==}. No
     * row here needs one yet, and a spelling invented for a row that does not exist would be
     * checked by nothing.
     */
    private static String asC(ObservedValue given) {
        return switch (given) {
            case ObservedValue.Integer it -> Long.toString(it.value());
            case ObservedValue.Bool it -> it.value() ? "1" : "0";
            default -> throw new AssertionError("no harness stands in over a " + given + " yet");
        };
    }

    private static String hex(byte[] bytes) {
        StringBuilder out = new StringBuilder(bytes.length * 2);
        for (byte each : bytes) {
            out.append("%02x".formatted(each & 0xff));
        }
        return out.toString();
    }

    private static ObservedValue read(Type answers, String said) {
        return switch (prim(answers)) {
            case INT -> new ObservedValue.Integer(Long.parseLong(said));
            case BOOL -> new ObservedValue.Bool(!"0".equals(said));
            case STRING -> new ObservedValue.Text(text(said));
            default -> throw new AssertionError("no harness reads a " + answers + " back yet");
        };
    }

    private static String text(String hex) {
        byte[] bytes = new byte[hex.length() / 2];
        for (int at = 0; at < bytes.length; at++) {
            bytes[at] = (byte) Integer.parseInt(hex.substring(2 * at, 2 * at + 2), 16);
        }
        return new String(bytes, StandardCharsets.UTF_8);
    }

    private static Type.Prim prim(Type type) {
        if (type instanceof Type.Prim it) {
            return it;
        }
        throw new AssertionError("no harness crosses a " + type + " yet");
    }

    @Override
    public void close() throws IOException {
        try (var walked = Files.walk(into)) {
            walked.sorted((a, b) -> b.getNameCount() - a.getNameCount()).forEach(it -> {
                try {
                    Files.deleteIfExists(it);
                } catch (IOException e) {
                    throw new UncheckedIOException(e);
                }
            });
        }
    }
}
