package souther.nativecode;

import souther.bindings.BindingApi;

import java.io.IOException;
import java.util.Set;
import java.util.TreeSet;
import java.util.jar.Attributes;
import java.util.jar.JarFile;
import java.util.jar.Manifest;
import java.util.regex.Pattern;

/**
 * What a generator's jar says of itself in its {@code META-INF/MANIFEST.MF}, read before any of its
 * code runs ({@link BindingApi}): which generator it is, the major of the API it was compiled
 * against, and the ABI generations the code it writes is for.
 */
record GeneratorDescriptor(String id, int api, Set<Integer> abiGenerations) {

    private static final Pattern ID = Pattern.compile("[a-z0-9][a-z0-9._-]*");
    private static final Pattern MAJOR = Pattern.compile("[1-9][0-9]{0,8}");
    private static final Pattern GENERATIONS = Pattern.compile("[1-9][0-9]{0,8}(,[1-9][0-9]{0,8})*");

    GeneratorDescriptor {
        abiGenerations = Set.copyOf(abiGenerations);
    }

    /** What {@code jar} says, strictly: an attribute missing, or not as it is to be written, refused. */
    static GeneratorDescriptor read(VerifiedJar jar) throws NotAGenerator {
        Manifest manifest;
        try (JarFile file = new JarFile(jar.jar().toFile(), false)) {
            manifest = file.getManifest();
        } catch (IOException e) {
            throw new NotAGenerator(jar.ref() + " is not a jar: " + e.getMessage(), e);
        }
        if (manifest == null) {
            throw new NotAGenerator(jar.ref() + " has no META-INF/MANIFEST.MF, so it says nothing of"
                    + " which generator it is");
        }
        Attributes main = manifest.getMainAttributes();
        String id = attribute(jar, main, BindingApi.ID, ID);
        String api = attribute(jar, main, BindingApi.API, MAJOR);
        String generations = attribute(jar, main, BindingApi.ABI_GENERATIONS, GENERATIONS);
        Set<Integer> abi = new TreeSet<>();
        for (String each : generations.split(",")) {
            if (!abi.add(Integer.parseInt(each))) {
                throw new NotAGenerator(jar.ref() + " names ABI generation " + each + " twice in "
                        + BindingApi.ABI_GENERATIONS);
            }
        }
        return new GeneratorDescriptor(id, Integer.parseInt(api), abi);
    }

    private static String attribute(VerifiedJar jar, Attributes main, String name, Pattern form)
            throws NotAGenerator {
        String value = main.getValue(name);
        if (value == null) {
            throw new NotAGenerator(jar.ref() + " says no " + name + " in its manifest");
        }
        if (!form.matcher(value).matches()) {
            throw new NotAGenerator(jar.ref() + " says " + name + ": \"" + value + "\", which is not"
                    + " one; it is to match " + form.pattern());
        }
        return value;
    }

    /**
     * Refuses a jar this command does not run: one whose id {@code rule} does not allow, written
     * against another major of the API, or for ABI generations that leave out the one this writes.
     */
    void check(IdRule rule, VerifiedJar jar) throws NotAGenerator {
        rule.check(id, jar.ref());
        if (api != BindingApi.MAJOR) {
            throw new NotAGenerator("the generator " + id + " (" + jar.ref() + ") is written against"
                    + " major " + api + " of souther-bindings-api, and this command runs major "
                    + BindingApi.MAJOR);
        }
        if (!abiGenerations.contains(ManifestReader.ABI)) {
            throw new NotAGenerator("the generator " + id + " (" + jar.ref() + ") writes code for ABI"
                    + " generations " + abiGenerations + ", and this command builds libraries of"
                    + " generation " + ManifestReader.ABI);
        }
    }
}
