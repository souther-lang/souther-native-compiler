package souther.nativecode;

import org.junit.jupiter.api.Test;

import java.io.IOException;
import java.nio.file.Files;
import java.util.ArrayList;
import java.util.List;
import java.util.TreeSet;
import java.util.regex.Matcher;
import java.util.regex.Pattern;

import static org.assertj.core.api.Assertions.assertThat;

/**
 * The release deploys the whole reactor, so what it publishes is whatever a module does not opt out
 * of. What is to be published is the compiler, the API, the testkit a generator's author tests with,
 * the generators the catalog names, and the pom they share; a module that says it is not published (the launcher, which a clone runs the command
 * with) is to say it to Maven as well, and the sentence and the setting are held to each other.
 */
class WhatTheReleasePublishesIsWhatTheModulesSayTest {

    private static final Pattern MODULE = Pattern.compile("<module>([^<]+)</module>");
    private static final Pattern ARTIFACT = Pattern.compile("</parent>\\s*<artifactId>([^<]+)</artifactId>");
    private static final Pattern ROOT = Pattern.compile("<artifactId>([^<]+)</artifactId>");

    private record Module(String artifact, boolean sayingNotPublished, boolean skipsDeploy) {
    }

    @Test
    void aModuleThatSaysItIsNotPublishedIsNotDeployed() throws IOException {
        for (Module module : modules()) {
            assertThat(module.skipsDeploy()).as("%s: what it says and what it is told", module.artifact())
                    .isEqualTo(module.sayingNotPublished());
        }
    }

    @Test
    void whatIsDeployedIsTheParentTheApiTheTestkitTheCompilerAndTheGeneratorsTheCatalogNames() throws IOException {
        TreeSet<String> published = new TreeSet<>();
        published.add(rootArtifact());
        modules().stream().filter(module -> !module.skipsDeploy()).forEach(module -> published.add(module.artifact()));

        TreeSet<String> expected = new TreeSet<>(List.of(rootArtifact(), "souther-bindings-api",
                "souther-bindings-testkit", "souther-native-compiler"));
        KnownBindings.all().forEach(kind -> expected.add(kind.artifact().split(":")[1]));
        assertThat(published).isEqualTo(expected);
    }

    private static String rootArtifact() throws IOException {
        Matcher artifact = ROOT.matcher(Files.readString(Repository.file("pom.xml")));
        assertThat(artifact.find()).isTrue();
        return artifact.group(1);
    }

    private static List<Module> modules() throws IOException {
        Matcher listed = MODULE.matcher(Files.readString(Repository.file("pom.xml")));
        List<Module> modules = new ArrayList<>();
        while (listed.find()) {
            String pom = Files.readString(Repository.file(listed.group(1), "pom.xml"));
            Matcher artifact = ARTIFACT.matcher(pom);
            assertThat(artifact.find()).as("the artifact of %s", listed.group(1)).isTrue();
            modules.add(new Module(artifact.group(1), pom.contains("Not published"),
                    pom.contains("<maven.deploy.skip>true</maven.deploy.skip>")));
        }
        return modules;
    }
}
