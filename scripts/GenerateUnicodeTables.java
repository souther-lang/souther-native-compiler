import java.io.IOException;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.Path;
import java.security.MessageDigest;
import java.security.NoSuchAlgorithmException;
import java.util.ArrayList;
import java.util.List;
import java.util.Map;
import java.util.Set;
import java.util.TreeMap;
import java.util.TreeSet;

/**
 * Regenerates {@code native/crates/text/src/tables.rs} from the Unicode Character Database.
 *
 * <p>What the language says NFC and case conversion are is Unicode 18.0.0's (spec
 * §string-canonical, §string-case), read off the database and not off a platform's own tables:
 * the Rust crates that normalize are at whichever version they were last released at, and a
 * program's answer would move with a dependency bump. Souther's JVM runtime generates its own tables
 * from the same files ({@code bin/GenerateNormalizationTables.java} and
 * {@code bin/GenerateCaseTables.java} in souther-lang/souther), and the checksums written into the
 * output are the ones those tables state, so a reader can see both were made from the same bytes.
 *
 * <p>Reads, for one pinned version:
 * <ul>
 * <li>{@code UnicodeData.txt}: the canonical combining classes, the canonical decompositions (a
 * decomposition carrying a {@code <tag>} is a compatibility one and is not read), and the simple
 * case mappings;</li>
 * <li>{@code CompositionExclusions.txt}: the exclusions nothing in {@code UnicodeData.txt}
 * implies;</li>
 * <li>{@code DerivedNormalizationProps.txt}: {@code Full_Composition_Exclusion}, only to check the
 * exclusions this derives against the ones Unicode publishes;</li>
 * <li>{@code SpecialCasing.txt}: the unconditional full case mappings and the {@code Final_Sigma}
 * one, every language-tailored entry read and set aside;</li>
 * <li>{@code DerivedCoreProperties.txt}: {@code Cased} and {@code Case_Ignorable}, which
 * {@code Final_Sigma} is stated over.</li>
 * </ul>
 *
 * <p>Stops rather than writing a table that looks right: a file whose version header is not
 * {@link #UNICODE_VERSION}, a derived exclusion set that is not {@code Full_Composition_Exclusion},
 * and a {@code SpecialCasing.txt} condition that is neither a known language nor
 * {@code Final_Sigma} each end the run.
 *
 * <p>Not part of any build: a Unicode version is part of the specification, and moving it is a
 * change to what programs mean. Run from the repository root, with the files above downloaded from
 * {@code https://www.unicode.org/Public/18.0.0/ucd/}:
 *
 * <pre>java scripts/GenerateUnicodeTables.java &lt;ucd-directory&gt;</pre>
 *
 * <p>{@code NormalizationTest.txt} from the same directory is then read by souther-text's own test
 * ({@code SOUTHER_UCD=<ucd-directory> cargo test -p souther-text -- --ignored}).
 */
public final class GenerateUnicodeTables {

    private static final Path OUTPUT = Path.of("native/crates/text/src/tables.rs");
    private static final String UNICODE_VERSION = "18.0.0";
    private static final Set<String> KNOWN_TAILORING_LANGUAGES = Set.of("lt", "tr", "az");

    private GenerateUnicodeTables() {}

    public static void main(String[] args) throws IOException, NoSuchAlgorithmException {
        if (args.length != 1) {
            System.err.println("usage: java scripts/GenerateUnicodeTables.java <ucd-directory>");
            System.exit(1);
        }
        Path ucd = Path.of(args[0]);
        Path unicodeData = ucd.resolve("UnicodeData.txt");
        Path compositionExclusions = ucd.resolve("CompositionExclusions.txt");
        Path derivedNormalizationProps = ucd.resolve("DerivedNormalizationProps.txt");
        Path specialCasing = ucd.resolve("SpecialCasing.txt");
        Path derivedCoreProperties = ucd.resolve("DerivedCoreProperties.txt");
        for (Path headed : List.of(compositionExclusions, derivedNormalizationProps, specialCasing,
                derivedCoreProperties)) {
            checkVersionHeader(headed);
        }

        Map<Integer, Integer> combiningClass = new TreeMap<>();
        Map<Integer, int[]> decomposition = new TreeMap<>();
        Map<Integer, Integer> simpleLower = new TreeMap<>();
        Map<Integer, Integer> simpleUpper = new TreeMap<>();
        readUnicodeData(unicodeData, combiningClass, decomposition, simpleLower, simpleUpper);

        Set<Integer> excluded = derivedExclusions(
                readCodePoints(compositionExclusions), decomposition, combiningClass);
        Set<Integer> published = readProperty(derivedNormalizationProps, "Full_Composition_Exclusion");
        if (!excluded.equals(published)) {
            throw new IllegalStateException("the exclusions derived here are not Unicode's "
                    + "Full_Composition_Exclusion: " + difference(excluded, published));
        }
        Map<Long, Integer> composition = new TreeMap<>();
        for (Map.Entry<Integer, int[]> entry : decomposition.entrySet()) {
            int[] parts = entry.getValue();
            if (parts.length == 2 && !excluded.contains(entry.getKey())) {
                composition.put(((long) parts[0] << 32) | parts[1], entry.getKey());
            }
        }

        Map<Integer, int[]> fullLower = new TreeMap<>();
        Map<Integer, int[]> fullUpper = new TreeMap<>();
        Map<Integer, int[]> finalSigma = new TreeMap<>();
        readSpecialCasing(specialCasing, fullLower, fullUpper, finalSigma);
        Map<Integer, int[]> lower = merged(simpleLower, fullLower);
        Map<Integer, int[]> upper = merged(simpleUpper, fullUpper);
        List<int[]> cased = readRanges(derivedCoreProperties, "Cased");
        List<int[]> caseIgnorable = readRanges(derivedCoreProperties, "Case_Ignorable");

        StringBuilder out = new StringBuilder();
        out.append("//! Unicode ").append(UNICODE_VERSION).append("'s tables for NFC and for default case conversion.\n");
        out.append("//!\n");
        out.append("//! Generated by `scripts/GenerateUnicodeTables.java` from the Unicode Character Database. Do\n");
        out.append("//! not edit: regenerate, which a Unicode version bump is the only reason to do. SHA-256 of the\n");
        out.append("//! files read, as downloaded from https://www.unicode.org/Public/").append(UNICODE_VERSION).append("/ucd/:\n");
        out.append("//!\n");
        for (Path read : List.of(unicodeData, compositionExclusions, derivedNormalizationProps,
                specialCasing, derivedCoreProperties)) {
            out.append("//! - ").append(read.getFileName()).append(": ").append(checksum(read)).append('\n');
        }
        out.append("\n// Every table is sorted by its first column, which is what the lookups search it by.\n\n");
        out.append("/// The Unicode version every table here is.\n");
        out.append("pub const UNICODE_VERSION: (u8, u8, u8) = (")
                .append(UNICODE_VERSION.replace('.', ',').replace(",", ", ")).append(");\n\n");

        out.append("/// Each code point whose canonical combining class is not nought, with its class.\n");
        out.append("pub(crate) static COMBINING_CLASS: &[(u32, u8)] = &[\n");
        for (Map.Entry<Integer, Integer> entry : combiningClass.entrySet()) {
            out.append("    (").append(hex(entry.getKey())).append(", ").append(entry.getValue()).append("),\n");
        }
        out.append("];\n\n");

        out.append("/// Each code point with a canonical decomposition, and the one step of it `UnicodeData.txt`\n");
        out.append("/// states: decomposing fully is decomposing what this answers again.\n");
        out.append("pub(crate) static DECOMPOSITION: &[(u32, &[u32])] = &[\n");
        mappings(out, decomposition);
        out.append("];\n\n");

        out.append("/// Each pair a primary composite is canonically decomposed into, with the composite: every\n");
        out.append("/// canonical decomposition of two, less `Full_Composition_Exclusion`.\n");
        out.append("pub(crate) static COMPOSITION: &[((u32, u32), u32)] = &[\n");
        for (Map.Entry<Long, Integer> entry : composition.entrySet()) {
            long pair = entry.getKey();
            out.append("    ((").append(hex((int) (pair >>> 32))).append(", ").append(hex((int) pair))
                    .append("), ").append(hex(entry.getValue())).append("),\n");
        }
        out.append("];\n\n");

        out.append("/// The full lowercase mapping of each code point that has one: `SpecialCasing.txt`'s\n");
        out.append("/// unconditional mapping where it states one, and `UnicodeData.txt`'s simple one otherwise.\n");
        out.append("pub(crate) static LOWERCASE: &[(u32, &[u32])] = &[\n");
        mappings(out, lower);
        out.append("];\n\n");

        out.append("/// The full uppercase mapping, read as [`LOWERCASE`] is.\n");
        out.append("pub(crate) static UPPERCASE: &[(u32, &[u32])] = &[\n");
        mappings(out, upper);
        out.append("];\n\n");

        out.append("/// The lowercase mapping of each code point that has one where `Final_Sigma` holds.\n");
        out.append("pub(crate) static FINAL_SIGMA: &[(u32, &[u32])] = &[\n");
        mappings(out, finalSigma);
        out.append("];\n\n");

        out.append("/// The runs of code points that are `Cased`, both ends in each.\n");
        out.append("pub(crate) static CASED: &[(u32, u32)] = &[\n");
        ranges(out, cased);
        out.append("];\n\n");

        out.append("/// The runs of code points that are `Case_Ignorable`, both ends in each.\n");
        out.append("pub(crate) static CASE_IGNORABLE: &[(u32, u32)] = &[\n");
        ranges(out, caseIgnorable);
        out.append("];\n");

        Files.writeString(OUTPUT, out.toString(), StandardCharsets.UTF_8);
        System.out.println("wrote " + OUTPUT + ": " + combiningClass.size() + " combining classes, "
                + decomposition.size() + " decompositions, " + composition.size() + " compositions, "
                + lower.size() + " lowercase, " + upper.size() + " uppercase, " + finalSigma.size()
                + " Final_Sigma, " + cased.size() + " Cased runs, " + caseIgnorable.size()
                + " Case_Ignorable runs");
    }

    private static void checkVersionHeader(Path path) throws IOException {
        String name = path.getFileName().toString().replace(".txt", "");
        String first = Files.readAllLines(path, StandardCharsets.UTF_8).getFirst();
        String expected = "# " + name + "-" + UNICODE_VERSION + ".txt";
        if (!first.equals(expected)) {
            throw new IllegalStateException(path + " opens with " + first + " and not " + expected);
        }
    }

    /** Fields, from nought: 3 the combining class, 5 the decomposition, 12 the simple uppercase
     *  mapping, 13 the simple lowercase one. A range written as a first and a last line
     *  ({@code <CJK Ideograph, First>}) has none of them, so its two lines are all it needs. */
    private static void readUnicodeData(Path path, Map<Integer, Integer> combiningClass,
            Map<Integer, int[]> decomposition, Map<Integer, Integer> lower,
            Map<Integer, Integer> upper) throws IOException {
        for (String line : Files.readAllLines(path, StandardCharsets.UTF_8)) {
            if (line.isBlank()) {
                continue;
            }
            String[] field = line.split(";", -1);
            int point = Integer.parseInt(field[0], 16);
            int ccc = Integer.parseInt(field[3]);
            if (ccc != 0) {
                combiningClass.put(point, ccc);
            }
            if (!field[5].isBlank() && !field[5].startsWith("<")) {
                decomposition.put(point, codePoints(field[5]));
            }
            if (!field[12].isBlank()) {
                upper.put(point, Integer.parseInt(field[12], 16));
            }
            if (!field[13].isBlank()) {
                lower.put(point, Integer.parseInt(field[13], 16));
            }
        }
    }

    /** What {@code Full_Composition_Exclusion} is made of: the script-specific and post-composition
     *  version exclusions stated outright, every singleton decomposition, and every decomposition
     *  that begins with a code point that is not a starter, or of a code point that is not one. */
    private static Set<Integer> derivedExclusions(Set<Integer> stated,
            Map<Integer, int[]> decomposition, Map<Integer, Integer> combiningClass) {
        Set<Integer> excluded = new TreeSet<>(stated);
        for (Map.Entry<Integer, int[]> entry : decomposition.entrySet()) {
            int[] parts = entry.getValue();
            if (parts.length == 1 || combiningClass.containsKey(entry.getKey())
                    || combiningClass.containsKey(parts[0])) {
                excluded.add(entry.getKey());
            }
        }
        return excluded;
    }

    private static String difference(Set<Integer> derived, Set<Integer> published) {
        Set<Integer> onlyDerived = new TreeSet<>(derived);
        onlyDerived.removeAll(published);
        Set<Integer> onlyPublished = new TreeSet<>(published);
        onlyPublished.removeAll(derived);
        return "only derived " + onlyDerived + ", only published " + onlyPublished;
    }

    private static void readSpecialCasing(Path path, Map<Integer, int[]> lower,
            Map<Integer, int[]> upper, Map<Integer, int[]> finalSigma) throws IOException {
        for (String raw : Files.readAllLines(path, StandardCharsets.UTF_8)) {
            String line = raw.replaceFirst("#.*", "");
            if (line.isBlank()) {
                continue;
            }
            String[] field = line.split(";", -1);
            int point = Integer.parseInt(field[0].trim(), 16);
            String condition = field.length > 5 ? field[4].trim() : "";
            if (condition.isEmpty()) {
                lower.put(point, codePoints(field[1]));
                upper.put(point, codePoints(field[3]));
            } else if (condition.equals("Final_Sigma")) {
                finalSigma.put(point, codePoints(field[1]));
            } else if (!KNOWN_TAILORING_LANGUAGES.contains(condition.split(" ")[0])) {
                throw new IllegalStateException("a SpecialCasing.txt condition this does not know, "
                        + condition + " at " + hex(point) + ": a context the default algorithm"
                        + " reads, or a language to set aside, is decided before this is run again");
            }
        }
    }

    /** The full mapping where one is stated, the simple one otherwise. */
    private static Map<Integer, int[]> merged(Map<Integer, Integer> simple, Map<Integer, int[]> full) {
        Map<Integer, int[]> merged = new TreeMap<>();
        simple.forEach((point, mapped) -> merged.put(point, new int[] {mapped}));
        merged.putAll(full);
        return merged;
    }

    private static Set<Integer> readCodePoints(Path path) throws IOException {
        Set<Integer> read = new TreeSet<>();
        for (String raw : Files.readAllLines(path, StandardCharsets.UTF_8)) {
            String line = raw.replaceFirst("#.*", "").trim();
            if (!line.isEmpty()) {
                int[] run = run(line);
                for (int point = run[0]; point <= run[1]; point++) {
                    read.add(point);
                }
            }
        }
        return read;
    }

    private static Set<Integer> readProperty(Path path, String property) throws IOException {
        Set<Integer> read = new TreeSet<>();
        for (int[] run : readRanges(path, property)) {
            for (int point = run[0]; point <= run[1]; point++) {
                read.add(point);
            }
        }
        return read;
    }

    /** {@code <run> ; <property>}, the runs merged where one ends where the next begins. */
    private static List<int[]> readRanges(Path path, String property) throws IOException {
        List<int[]> runs = new ArrayList<>();
        for (String raw : Files.readAllLines(path, StandardCharsets.UTF_8)) {
            String[] field = raw.replaceFirst("#.*", "").split(";");
            if (field.length >= 2 && field[1].trim().equals(property)) {
                runs.add(run(field[0].trim()));
            }
        }
        runs.sort((a, b) -> Integer.compare(a[0], b[0]));
        List<int[]> merged = new ArrayList<>();
        for (int[] run : runs) {
            if (!merged.isEmpty() && merged.getLast()[1] + 1 >= run[0]) {
                merged.getLast()[1] = Math.max(merged.getLast()[1], run[1]);
            } else {
                merged.add(run.clone());
            }
        }
        return merged;
    }

    private static int[] run(String written) {
        int dots = written.indexOf("..");
        if (dots < 0) {
            int point = Integer.parseInt(written, 16);
            return new int[] {point, point};
        }
        return new int[] {Integer.parseInt(written.substring(0, dots), 16),
                Integer.parseInt(written.substring(dots + 2), 16)};
    }

    private static int[] codePoints(String written) {
        String[] parts = written.trim().split("\\s+");
        int[] points = new int[parts.length];
        for (int at = 0; at < parts.length; at++) {
            points[at] = Integer.parseInt(parts[at], 16);
        }
        return points;
    }

    private static void mappings(StringBuilder out, Map<Integer, int[]> mapped) {
        for (Map.Entry<Integer, int[]> entry : mapped.entrySet()) {
            out.append("    (").append(hex(entry.getKey())).append(", &[");
            int[] to = entry.getValue();
            for (int at = 0; at < to.length; at++) {
                out.append(at == 0 ? "" : ", ").append(hex(to[at]));
            }
            out.append("]),\n");
        }
    }

    private static void ranges(StringBuilder out, List<int[]> runs) {
        for (int[] run : runs) {
            out.append("    (").append(hex(run[0])).append(", ").append(hex(run[1])).append("),\n");
        }
    }

    private static String hex(int point) {
        return String.format("0x%04X", point);
    }

    private static String checksum(Path path) throws IOException, NoSuchAlgorithmException {
        byte[] hash = MessageDigest.getInstance("SHA-256").digest(Files.readAllBytes(path));
        StringBuilder hex = new StringBuilder();
        for (byte each : hash) {
            hex.append(String.format("%02x", each));
        }
        return hex.toString();
    }
}
