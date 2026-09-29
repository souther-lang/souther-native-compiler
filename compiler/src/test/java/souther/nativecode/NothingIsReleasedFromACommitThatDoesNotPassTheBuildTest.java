package souther.nativecode;

import org.junit.jupiter.api.Test;

import java.io.IOException;
import java.nio.file.Files;
import java.util.HashMap;
import java.util.LinkedHashMap;
import java.util.Map;
import java.util.regex.Matcher;
import java.util.regex.Pattern;

import static org.assertj.core.api.Assertions.assertThat;

/**
 * A tag can be put on any commit, and a release is not replaced once it is out: every user of the
 * version fetches from it. So the release runs the build on the commit it is made from, and does
 * nothing that publishes or builds a release artifact unless that passed. Nothing runs the release
 * before a tag, so this is held here, and not found by a release that went out from a commit whose
 * tests were never run.
 *
 * <p>Read from the workflow: it calls the build's workflow as its first job, and every other job
 * depends on it, directly or through another.
 */
class NothingIsReleasedFromACommitThatDoesNotPassTheBuildTest {

    private static final String BUILD = "./.github/workflows/build.yml";
    private static final Pattern JOB = Pattern.compile("^  ([a-z][a-z-]*):\\s*$", Pattern.MULTILINE);
    private static final Pattern NEEDS = Pattern.compile("^    needs: (\\S+)\\s*$", Pattern.MULTILINE);

    @Test
    void theBuildCanBeCalledByTheRelease() throws IOException {
        assertThat(Files.readString(Repository.file(".github", "workflows", "build.yml")))
                .contains("workflow_call:");
    }

    @Test
    void theReleaseCallsTheBuildAsItsFirstJob() throws IOException {
        Map<String, String> jobs = jobs();

        assertThat(jobs).isNotEmpty();
        String first = jobs.keySet().iterator().next();
        assertThat(jobs.get(first)).as("the first job of the release").contains("uses: " + BUILD);
        assertThat(needs(jobs.get(first))).as("and it waits for nothing").isNull();
    }

    @Test
    void everyOtherJobOfTheReleaseWaitsForItThroughTheOnesBetween() throws IOException {
        Map<String, String> jobs = jobs();
        String build = jobs.keySet().iterator().next();

        for (Map.Entry<String, String> job : jobs.entrySet()) {
            if (job.getKey().equals(build)) {
                continue;
            }
            String at = job.getKey();
            for (int steps = 0; !at.equals(build); steps++) {
                assertThat(steps).as("%s reaches the build through what it needs", job.getKey())
                        .isLessThan(jobs.size());
                String next = needs(jobs.get(at));
                assertThat(next).as("what %s waits for", at).isNotNull();
                assertThat(jobs).as("%s needs a job of this workflow", at).containsKey(next);
                at = next;
            }
        }
    }

    /** The jobs of the release in the order they are written, by their text. */
    private static Map<String, String> jobs() throws IOException {
        String workflow = Files.readString(Repository.file(".github", "workflows", "release.yml"));
        String body = workflow.substring(workflow.indexOf("\njobs:\n") + "\njobs:\n".length());
        Map<String, Integer> starts = new LinkedHashMap<>();
        Matcher job = JOB.matcher(body);
        while (job.find()) {
            starts.put(job.group(1), job.start());
        }
        Map<String, String> texts = new LinkedHashMap<>();
        Map<String, Integer> ends = new HashMap<>();
        String previous = null;
        for (Map.Entry<String, Integer> each : starts.entrySet()) {
            if (previous != null) {
                ends.put(previous, each.getValue());
            }
            previous = each.getKey();
        }
        if (previous != null) {
            ends.put(previous, body.length());
        }
        starts.forEach((name, start) -> texts.put(name, body.substring(start, ends.get(name))));
        return texts;
    }

    private static String needs(String job) {
        Matcher needs = NEEDS.matcher(job);
        return needs.find() ? needs.group(1) : null;
    }
}
