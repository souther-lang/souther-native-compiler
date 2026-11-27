package souther.nativecode;

import souther.compiler.command.Environment;

import java.io.PrintStream;

/**
 * The environment a test runs the command in: this process's, as the build started it, with the
 * streams the test reads back in place of the process's own.
 *
 * <p>Read from the process each time it is asked, so a test that names a driver or a generator by a
 * property, or takes one away, runs the command with the properties as they then stand.
 */
final class ThisProcess {

    private ThisProcess() {
    }

    /** The command {@code args} say, their paths read from the directory this process runs in. */
    static Main.Command read(String[] args) throws Main.NotACommand {
        return Main.read(args, Environment.ofThisProcess());
    }

    /** This process's environment, writing to {@code out} and {@code err}. */
    static Environment writing(PrintStream out, PrintStream err) {
        Environment process = Environment.ofThisProcess();
        return new Environment(process.variables(), process.properties(),
                process.workingDirectory(), process.in(), out, err, false, process.today());
    }
}
