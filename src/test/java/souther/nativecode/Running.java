package souther.nativecode;

import souther.compiler.observe.ObservedValue;
import souther.compiler.observe.StoodIn;
import souther.compiler.program.CheckedBehavior;
import souther.compiler.program.CheckedModule;
import souther.compiler.program.CheckedProgram;
import souther.compiler.program.CheckedSignature;
import souther.compiler.program.StandsIn;
import souther.compiler.types.Type;

import java.io.IOException;
import java.io.UncheckedIOException;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.Path;
import java.util.ArrayList;
import java.util.HashMap;
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
 * <p>The spelling of a symbol is written here a second time, which is the one place in this project
 * it is. The compiler hands Cranelift the name from the {@code abi} crate and never sees it again;
 * a C declaration is the only other thing that has to say it, and it has to say it in C.
 */
final class Running implements AutoCloseable {

    private final CheckedProgram program;
    private final Path into;
    private final Path object;
    private final Map<String, Path> linked = new HashMap<>();

    private Running(CheckedProgram program, Path into, Path object) {
        this.program = program;
        this.into = into;
        this.object = object;
    }

    /** The program compiled to one object, with nothing linked yet. */
    static Running of(CheckedProgram program) throws IOException, InterruptedException {
        Path into = Files.createTempDirectory("souther-native-running");
        Path object = into.resolve("program.o");
        Files.write(object, NativeCompiler.compile(program));
        return new Running(program, into, object);
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
        Path executable = linked(module, behavior, standIns);
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
        return Optional.of(read(behavior.signature().answers(), said.strip()));
    }

    private Path linked(CheckedModule module, CheckedBehavior behavior, List<StandsIn> standIns)
            throws IOException {
        String reached = module.name() + "." + behavior.name().name();
        // What stands in for a dependency is part of what is linked, so two rows of one behavior
        // that state different stand-ins are two executables and not one reused.
        String name = standIns.isEmpty()
                ? reached
                : reached + "." + Integer.toHexString(standIns.toString().hashCode());
        Path already = linked.get(name);
        if (already != null) {
            return already;
        }

        Path harness = into.resolve(name + ".c");
        Files.writeString(harness, harnessFor("souther." + reached, behavior, standIns),
                StandardCharsets.UTF_8);
        Path executable = into.resolve(name);

        Process cc = new ProcessBuilder("cc", "-o", executable.toString(),
                harness.toString(), object.toString(), RUNTIME.toString())
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
        linked.put(name, executable);
        return executable;
    }

    /** What a program made here calls for room, linked in beside what this compiler emitted. */
    private static final Path RUNTIME =
            Path.of("native", "target", "debug", "libsouther_native_runtime.a");

    /**
     * A C program that reaches the one behavior and writes what it answered.
     *
     * <p>Written for this behavior's own signature rather than dispatched at run time: the arity
     * and the widths are what a call is made of, and a harness that took them as data would be
     * making the call from something other than what the behavior says it takes.
     *
     * <p>The call is bracketed, which is how a Souther value is freed: nothing frees one on its
     * own, and what the run made is dropped in a single go by the caller. Bracketed here even
     * where one call is all that happens, because a harness that never gave the room back would be
     * a harness the contract had never been put to.
     */
    private String harnessFor(String symbol, CheckedBehavior behavior,
                              List<StandsIn> standIns) {
        List<Type> takes = behavior.signature().takes();
        StringJoiner parameters = new StringJoiner(", ");
        StringJoiner arguments = new StringJoiner(", ");
        for (int at = 0; at < takes.size(); at++) {
            parameters.add(cType(takes.get(at)));
            arguments.add(read(takes.get(at), at + 1));
        }

        StringJoiner supplied = new StringJoiner("\n");
        for (StandsIn standsIn : standIns) {
            supplied.add(standingIn(standsIn));
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
                cType(behavior.signature().answers()),
                takes.isEmpty() ? "void" : parameters.toString(),
                PREFIX, symbol,
                takes.size() + 1,
                cType(behavior.signature().answers()),
                arguments.toString(),
                format(behavior.signature().answers()));
    }

    /**
     * What answers a behavior the object names and does not define.
     *
     * <p>The row says what the dependency answers, entry by entry, and this is that table as the
     * definition the linker was missing. Arguments it was not told about end the run rather than
     * answering something: a stand-in asked for what the row never stated would be this harness
     * deciding what the dependency does, which is the row's to say.
     */
    private String standingIn(StandsIn standsIn) {
        StoodIn stated = standsIn.stated();
        CheckedSignature signature = program.behavior(standsIn.dependency()).signature();
        List<Type> takes = signature.takes();
        StringJoiner parameters = new StringJoiner(", ");
        for (int at = 0; at < takes.size(); at++) {
            parameters.add(cType(takes.get(at)) + " a" + at);
        }

        StringBuilder answering = new StringBuilder();
        for (StoodIn.Entry entry : stated.entries()) {
            StringJoiner asked = new StringJoiner(" && ");
            for (int at = 0; at < entry.arguments().size(); at++) {
                asked.add("a" + at + " == " + written(entry.arguments().get(at)));
            }
            answering.append("    if (%s) { return %s; }\n"
                    .formatted(asked.toString(), written(entry.answer())));
        }
        String otherwise = switch (stated.otherwise()) {
            case StoodIn.Otherwise.Answer it -> "    return " + written(it.value()) + ";\n";
            case StoodIn.Otherwise.NothingStated it -> "    exit(3);\n";
        };

        String symbol = PREFIX + "souther." + standsIn.dependency().module()
                + "." + standsIn.dependency().name();
        return "%s standsIn(%s) __asm__(\"%s\");\n%s standsIn(%s) {\n%s%s}"
                .formatted(cType(signature.answers()), parameters, symbol,
                        cType(signature.answers()), parameters, answering, otherwise);
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
