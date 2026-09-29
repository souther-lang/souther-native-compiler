package souther.nativecode;

import org.junit.jupiter.api.Test;

import java.io.IOException;
import java.nio.file.Files;
import java.nio.file.Path;
import java.util.LinkedHashMap;
import java.util.List;
import java.util.Map;
import java.util.regex.Matcher;
import java.util.regex.Pattern;
import java.util.stream.Stream;

import static org.assertj.core.api.Assertions.assertThat;

/**
 * A job of a workflow is a runner of its own, so what one installed is not there in another: the
 * release does not have the Go the build installed. What a job needs is therefore a fact about each
 * job, and is held for each, and not remembered from the one that happened to have it.
 *
 * <p>The Go a job needs is said once, in the {@code go} line of the runtime module's go.mod: a job
 * installs the Go that file names, and a script that runs Go asks whether the Go it has is that
 * ({@code scripts/require-go.sh}) and never lets Go fetch another. Held here from the text of the
 * workflows and the scripts, since nothing runs a release before a tag.
 */
class EveryJobThatRunsGoHasTheGoTheRuntimeModuleNamesTest {

    private static final Pattern JOB = Pattern.compile("^  ([a-z][a-z-]*):\\s*$", Pattern.MULTILINE);
    private static final Pattern RUNS_GO = Pattern
            .compile("scripts/(go-from-the-command-line|verify-go-runtime-release|publish-go-runtime)\\.sh");
    private static final String GO_MOD = "go-version-file: bindings/go/runtime/go.mod";

    /** The jobs of a workflow, by their names, in the text they are written in. */
    private static Map<String, String> jobs(String workflow) throws IOException {
        String text = Files.readString(Repository.file(".github", "workflows", workflow));
        String body = text.substring(text.indexOf("\njobs:\n") + "\njobs:\n".length());
        List<int[]> starts = new java.util.ArrayList<>();
        List<String> names = new java.util.ArrayList<>();
        Matcher job = JOB.matcher(body);
        while (job.find()) {
            names.add(job.group(1));
            starts.add(new int[]{job.start()});
        }
        Map<String, String> jobs = new LinkedHashMap<>();
        for (int at = 0; at < names.size(); at++) {
            int end = at + 1 < names.size() ? starts.get(at + 1)[0] : body.length();
            jobs.put(names.get(at), body.substring(starts.get(at)[0], end));
        }
        return jobs;
    }

    @Test
    void aJobThatRunsWhatRunsGoInstallsTheGoTheRuntimeModuleNames() throws IOException {
        int held = 0;
        for (String workflow : List.of("build.yml", "release.yml")) {
            for (Map.Entry<String, String> job : jobs(workflow).entrySet()) {
                if (!RUNS_GO.matcher(job.getValue()).find()) {
                    continue;
                }
                held++;
                assertThat(job.getValue()).as("the job %s of %s runs Go", job.getKey(), workflow)
                        .contains("uses: actions/setup-go@").contains(GO_MOD);
            }
        }
        assertThat(held).as("the jobs that run Go, which are the build's and the release's").isEqualTo(2);
    }

    @Test
    void noWorkflowSaysWhichGoInAnyWordsOfItsOwn() throws IOException {
        for (String workflow : List.of("build.yml", "release.yml")) {
            assertThat(Files.readString(Repository.file(".github", "workflows", workflow)))
                    .as(workflow).doesNotContain("go-version:");
        }
    }

    @Test
    void aScriptThatRunsGoAsksWhetherItHasTheGoItNeeds() throws IOException {
        Pattern goCommand = Pattern.compile("(?m)^[^#\\n]*(?:^|[\\s(`$])go (?:mod|run|vet|test|build|env|list)\\b");
        int held = 0;
        try (Stream<Path> scripts = Files.list(Repository.file("scripts"))) {
            for (Path script : scripts.filter(it -> it.toString().endsWith(".sh")).toList()) {
                if (script.getFileName().toString().equals("require-go.sh")) {
                    continue;
                }
                String text = Files.readString(script);
                if (goCommand.matcher(text).find()) {
                    held++;
                    assertThat(text).as("%s runs Go", script.getFileName()).contains("require-go.sh");
                }
            }
        }
        assertThat(held).as("the scripts that run Go").isEqualTo(4);
    }

    @Test
    void theGuardHoldsGoToTheToolchainInstalledAndToWhatTheModuleNames() throws IOException {
        String guard = Files.readString(Repository.file("scripts", "require-go.sh"));

        assertThat(guard).contains("export GOTOOLCHAIN=local").contains("bindings/go/runtime/go.mod");
    }
}
