package souther.nativecode;

import org.junit.jupiter.api.Test;

import java.io.IOException;
import java.io.UncheckedIOException;
import java.nio.file.Files;
import java.nio.file.Path;
import java.util.ArrayList;
import java.util.List;
import java.util.regex.Matcher;
import java.util.regex.Pattern;
import java.util.stream.Stream;

import static org.assertj.core.api.Assertions.assertThat;

/**
 * A test runner a module starts through exec-maven-plugin in the test phase (Cargo's, Go's) is skipped
 * where surefire is: by {@code -DskipTests}. exec-maven-plugin does not read that property itself, so
 * a runner bound without it ran its tests on every build that asked for none, and
 * scripts/souther-native, which builds the clone that way before every command, ran Cargo's and Go's
 * tests six times over in each CI run after the build had already run them.
 */
class EveryTestRunnerMavenStartsIsSkippedWithItsTestsTest {

    private static final Pattern EXECUTION =
            Pattern.compile("<execution>(.*?)</execution>", Pattern.DOTALL);

    private static final Pattern IN_THE_TEST_PHASE = Pattern.compile("<phase>test</phase>");

    /** exec-maven-plugin's goals: a program, or a Java main. */
    private static final Pattern RUN = Pattern.compile("<goal>(exec|java)</goal>");

    private static final Pattern ID = Pattern.compile("<id>([^<]+)</id>");

    private static final String SKIPPED = "<skip>${skipTests}</skip>";

    @Test
    void everyExecutionInTheTestPhaseIsSkippedWithTheTests() {
        List<String> runningAnyway = new ArrayList<>();
        int executions = 0;
        for (Path pom : poms()) {
            Matcher execution = EXECUTION.matcher(read(pom));
            while (execution.find()) {
                String body = execution.group(1);
                if (IN_THE_TEST_PHASE.matcher(body).find() && RUN.matcher(body).find()) {
                    executions++;
                    if (!body.contains(SKIPPED)) {
                        Matcher id = ID.matcher(body);
                        runningAnyway.add(Repository.root().relativize(pom) + ": "
                                + (id.find() ? id.group(1) : body.strip()));
                    }
                }
            }
        }
        assertThat(executions).as("test-phase executions found").isPositive();
        assertThat(runningAnyway).as("test-phase executions not `%s`", SKIPPED).isEmpty();
    }

    /** The root pom and every module's, wherever a module sits. */
    private static List<Path> poms() {
        try (Stream<Path> files = Files.walk(Repository.root())) {
            return files.filter(it -> it.getFileName().toString().equals("pom.xml"))
                    .filter(it -> Repository.root().relativize(it).toString()
                            .matches("(?!.*(^|/)(target|\\.git|\\.wt|vendor|node_modules)/).*"))
                    .sorted().toList();
        } catch (IOException e) {
            throw new UncheckedIOException(e);
        }
    }

    private static String read(Path pom) {
        try {
            return Files.readString(pom);
        } catch (IOException e) {
            throw new UncheckedIOException(e);
        }
    }
}
