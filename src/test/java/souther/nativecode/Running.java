package souther.nativecode;

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
import java.util.Optional;
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
     * <p>An abort is not an answer and does not come back as one: the run ends, and what a caller
     * of this is holding is a row that states a value, so the run ending is that row not holding
     * rather than a value to compare.
     */
    ObservedValue answering(CheckedModule module, CheckedBehavior behavior,
                            List<ObservedValue> inputs) throws IOException, InterruptedException {
        return answering(module, behavior, inputs, List.of());
    }

    ObservedValue answering(CheckedModule module, CheckedBehavior behavior,
                            List<ObservedValue> inputs, List<StandsIn> standIns)
            throws IOException, InterruptedException {
        return answeredOrEnded(module, behavior, inputs, standIns).orElseThrow(
                () -> new AssertionError("the run ended rather than answering: " + behavior.name()
                        + " of " + module.name() + ", handed " + inputs));
    }

    /**
     * What the behavior answered, or nothing where the run ended instead.
     *
     * <p>For a caller whose question is which of the two happened. A run ending is an answer to
     * that question and not the absence of one, so it comes back as a value rather than as a
     * failure — and a caller asking it is expected to have a pair of runs where one of each
     * happens, since a run that ends says nothing on its own about why.
     */
    Optional<ObservedValue> answeredOrEnded(CheckedModule module, CheckedBehavior behavior,
                                            List<ObservedValue> inputs)
            throws IOException, InterruptedException {
        return answeredOrEnded(module, behavior, inputs, List.of());
    }

    Optional<ObservedValue> answeredOrEnded(CheckedModule module, CheckedBehavior behavior,
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
        return rowAnsweredOrEnded(module, behavior, at, standIns).orElseThrow(
                () -> new AssertionError("the run ended rather than answering row " + at + " of "
                        + behavior.name() + " of " + module.name()));
    }

    /**
     * What one of the behavior's rows answers, run through the entry the object carries for it.
     *
     * <p>Not the behavior handed the row's values: the values were written into the object when
     * the program crossed, and the entry is what the object offers to whoever runs its rows. Which
     * is what lets a row of a name the module keeps be run at all — a module's surface is what the
     * behavior's own symbol answers to, and a row is a different question.
     */
    Optional<ObservedValue> rowAnsweredOrEnded(CheckedModule module, CheckedBehavior behavior,
                                               int at, List<StandsIn> standIns)
            throws IOException, InterruptedException {
        String named = module.name() + "." + behavior.name().name() + ".example." + at;
        String symbol = "souther." + module.name() + "." + behavior.name().name()
                + "$example$" + at;
        Path executable = linked(named, symbol, behavior.signature().answers(), List.of(),
                standIns);
        return ran(executable, behavior.signature().answers(), List.of());
    }

    private Optional<ObservedValue> ran(Path executable, Type answers, List<ObservedValue> inputs)
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
            return Optional.empty();
        }
        return Optional.of(read(answers, said.strip()));
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

                %s

                extern %s reached(%s) __asm__("%s%s");
                extern int64_t souther_mark(void);
                extern void souther_reset(int64_t);

                int main(int argc, char **argv) {
                    if (argc != %d) {
                        return 2;
                    }
                    int64_t mark = souther_mark();
                    %s answered = reached(%s);
                    printf("%s\\n", answered);
                    souther_reset(mark);
                    return 0;
                }
                """.formatted(
                supplied.toString(),
                cType(answers),
                takenIn(taken),
                PREFIX, symbol,
                takes.size() + 1,
                cType(answers),
                String.join(", ", given),
                format(answers));
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
        String parameters = takenIn(taken);

        StringBuilder answering = new StringBuilder();
        String otherwise = "    exit(3);\n";
        if (standsIn != null) {
            StoodIn stated = standsIn.stated();
            for (StoodIn.Entry entry : stated.entries()) {
                List<String> asked = new ArrayList<>();
                for (int at = 0; at < entry.arguments().size(); at++) {
                    asked.add("a" + at + " == " + written(entry.arguments().get(at)));
                }
                answering.append("    if (%s) { return %s; }\n"
                        .formatted(everyOneOf(asked), written(entry.answer())));
            }
            otherwise = switch (stated.otherwise()) {
                case StoodIn.Otherwise.Answer it -> "    return " + written(it.value()) + ";\n";
                case StoodIn.Otherwise.NothingStated it -> "    exit(3);\n";
            };
        }

        String symbol = PREFIX + "souther." + dependency.module() + "." + dependency.name();
        return "%s %s(%s) __asm__(\"%s\");\n%s %s(%s) {\n%s%s}"
                .formatted(cType(signature.answers()), reached, parameters, symbol,
                        cType(signature.answers()), reached, parameters, answering, otherwise);
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

    private static String cType(Type type) {
        return switch (prim(type)) {
            case INT -> "int64_t";
            case BOOL -> "int8_t";
            default -> throw new AssertionError("no harness writes a " + type + " yet");
        };
    }

    private static String format(Type type) {
        return switch (prim(type)) {
            case INT -> "%\" PRId64 \"";
            case BOOL -> "%d";
            default -> throw new AssertionError("no harness writes a " + type + " yet");
        };
    }

    private static String read(Type type, int at) {
        return switch (prim(type)) {
            case INT -> "strtoll(argv[" + at + "], NULL, 10)";
            case BOOL -> "(int8_t) (strtoll(argv[" + at + "], NULL, 10) != 0)";
            default -> throw new AssertionError("no harness reads a " + type + " yet");
        };
    }

    private static String written(ObservedValue given) {
        return switch (given) {
            case ObservedValue.Integer it -> Long.toString(it.value());
            case ObservedValue.Bool it -> it.value() ? "1" : "0";
            default -> throw new AssertionError("no harness hands over a " + given + " yet");
        };
    }

    private static ObservedValue read(Type answers, String said) {
        return switch (prim(answers)) {
            case INT -> new ObservedValue.Integer(Long.parseLong(said));
            case BOOL -> new ObservedValue.Bool(!"0".equals(said));
            default -> throw new AssertionError("no harness reads a " + answers + " back yet");
        };
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
