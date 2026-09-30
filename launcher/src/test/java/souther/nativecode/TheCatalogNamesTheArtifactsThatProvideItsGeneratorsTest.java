package souther.nativecode;

import org.junit.jupiter.api.Test;

import java.io.IOException;
import java.nio.file.Files;
import java.nio.file.Path;
import java.util.ArrayList;
import java.util.List;
import java.util.regex.Matcher;
import java.util.regex.Pattern;

import static org.assertj.core.api.Assertions.assertThat;

/**
 * The catalog says which artifact brings each generator, and what a missing generator is fetched by
 * is that name. The modules say what each artifact is, in their own poms, so the same fact is written
 * twice, and nothing else would notice the two parting: a launcher takes its generators from its own
 * dependencies, and the catalog's name is not used on any path a build runs.
 *
 * <p>So for every binding the catalog names, an artifact of that name is a module of this
 * repository, and the generator it provides, the one found through its service file, is the one the
 * catalog's id asks for.
 */
class TheCatalogNamesTheArtifactsThatProvideItsGeneratorsTest {

    private static final Pattern MODULE = Pattern.compile("<module>([^<]+)</module>");
    private static final Pattern ARTIFACT = Pattern.compile("</parent>\\s*<artifactId>([^<]+)</artifactId>");
    private static final Pattern GROUP = Pattern.compile("<groupId>([^<]+)</groupId>");

    @Test
    void everyBindingIsBroughtByAnArtifactThatProvidesItsGenerator() throws Exception {
        Path root = Path.of(System.getProperty("souther.repository"));
        String group = GROUP.matcher(Files.readString(root.resolve("pom.xml")))
                .results().findFirst().orElseThrow().group(1);
        Matcher modules = MODULE.matcher(Files.readString(root.resolve("pom.xml")));
        List<Path> directories = new ArrayList<>();
        while (modules.find()) {
            directories.add(root.resolve(modules.group(1)));
        }

        for (KnownBindings.Kind kind : KnownBindings.all()) {
            assertThat(kind.artifact()).startsWith(group + ":");
            String artifact = kind.artifact().substring(group.length() + 1);
            assertThat(artifact).as("the release's scripts find a generator's jar by its id")
                    .isEqualTo("souther-binding-" + kind.id());
            List<Path> named = directories.stream().filter(it -> artifactOf(it).equals(artifact))
                    .toList();
            assertThat(named).as("the module that is %s", kind.artifact()).hasSize(1);

            Path services = named.getFirst()
                    .resolve("src/main/resources/META-INF/services/souther.bindings.BindingGenerator");
            assertThat(Files.readAllLines(services)).as("what %s provides", kind.artifact())
                    .contains(Bindings.installed().generatorFor(kind).getClass().getName());
        }
    }

    private static String artifactOf(Path module) {
        try {
            Matcher artifact = ARTIFACT.matcher(Files.readString(module.resolve("pom.xml")));
            return artifact.find() ? artifact.group(1) : "";
        } catch (IOException e) {
            throw new java.io.UncheckedIOException(e);
        }
    }
}
