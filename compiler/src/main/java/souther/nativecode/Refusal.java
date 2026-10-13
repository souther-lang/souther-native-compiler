package souther.nativecode;

import souther.compiler.cst.SourceLayout;
import souther.compiler.diag.CompileException;
import souther.compiler.diag.DiagnosticRenderer;
import souther.compiler.diag.HumanRenderer;
import souther.compiler.diag.SourceContext;
import souther.compiler.diag.SourceContextResolver;
import souther.compiler.query.Compilation;

import java.util.List;
import java.util.Locale;

/**
 * Why the language refused a program, as the language's human report: each diagnostic with the
 * source it points into named, the line quoted and a caret under the place.
 *
 * <p>Where a report points is a place in a text; a line and a column are what that text is laid out
 * as, so they belong to whoever holds the texts rather than to the exception. Whoever compiled them
 * hands them over here with the names a reader is shown them by. An error that carries no diagnostic
 * is a site not yet reporting through the catalog, and its message is the whole of what there is to
 * say about it.
 *
 * <p>No colour and English: what reads this is as often a log or a pipe as a terminal, and every
 * other sentence the command writes is English.
 */
public final class Refusal {

    private Refusal() {
    }

    /**
     * The lines of the report of {@code refused}, about the program compiled from {@code texts} in
     * that order, each shown under the name at the same position of {@code names}.
     */
    public static List<String> lines(CompileException refused, List<String> names,
                                     List<String> texts) {
        if (refused.diagnostic() == null) {
            return List.of(refused.getMessage());
        }
        return DiagnosticRenderer.renderAll(refused.locatedDiagnostics(), quoting(names, texts),
                new HumanRenderer(false), Locale.ENGLISH);
    }

    /**
     * What to quote for each source a report names: the text that was compiled, under its name.
     *
     * <p>A report names a source by the position it was handed over in. A name that is none of them
     * is about a source this compile did not hand over, one read from the class path, and answering
     * it with a text that happens to be here would draw a caret in a text the report says nothing
     * about. The layout is told which source its text is, so that it will not resolve a place from
     * another one.
     */
    private static SourceContextResolver quoting(List<String> names, List<String> texts) {
        return SourceContextResolver.memoized(id -> {
            for (int i = 0; i < texts.size(); i++) {
                if (Compilation.idOfSourceIndex(i).equals(id)) {
                    return new SourceContext(names.get(i), texts.get(i),
                            SourceLayout.of(texts.get(i), id));
                }
            }
            return null;
        });
    }
}
