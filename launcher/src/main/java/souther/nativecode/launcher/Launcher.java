package souther.nativecode.launcher;

import souther.compiler.command.Environment;
import souther.nativecode.Main;

import java.util.List;

/**
 * The command as a clone starts it: the backend's command, run with this process's arguments and
 * environment, and the process ended with what it answers.
 *
 * <p>The backend is no program of its own. A user's {@code souther compile --target native} loads
 * its jar into the souther command line's JVM, and that command line is what reads the process.
 * A clone builds against a Souther whose command line it may not have, so this stands where that
 * command line does, for the one target, with the compiler on the class path beside it.
 */
public final class Launcher {

    private Launcher() {
    }

    public static void main(String[] args) {
        System.exit(new Main().run(List.of(args), Environment.ofThisProcess()));
    }
}
