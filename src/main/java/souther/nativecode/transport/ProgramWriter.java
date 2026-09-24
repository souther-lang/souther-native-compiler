package souther.nativecode.transport;

import souther.compiler.abort.AbortKind;
import souther.compiler.abort.AbortSet;
import souther.compiler.core.Composition;
import souther.compiler.core.Contract;
import souther.compiler.core.Core;
import souther.compiler.core.EnsuresEnforcement;
import souther.compiler.core.Kernel;
import souther.compiler.core.ValueShape;
import souther.compiler.observe.ObservedValue;
import souther.compiler.observe.RowStatement;
import souther.compiler.program.BehaviorTarget;
import souther.compiler.program.CheckedAlternativesForm;
import souther.compiler.program.CheckedBoundaryInput;
import souther.compiler.program.CheckedBoundaryOutput;
import souther.compiler.program.CheckedCodecShape;
import souther.compiler.program.CheckedBehavior;
import souther.compiler.program.CheckedData;
import souther.compiler.program.CheckedHelper;
import souther.compiler.program.CheckedImplementation;
import souther.compiler.program.CheckedModule;
import souther.compiler.program.CheckedProgram;
import souther.compiler.program.CheckedRow;
import souther.compiler.program.CheckedValue;
import souther.compiler.program.CheckedValueEntry;
import souther.compiler.program.Declared;
import souther.compiler.program.DeclaredBy;
import souther.compiler.program.Publication;
import souther.compiler.types.BinOp;
import souther.compiler.types.BindingId;
import souther.compiler.types.LanguageCaseId;
import souther.compiler.types.LeafScalar;
import souther.compiler.types.MapKeyRepresentation;
import souther.compiler.types.Refinement;
import souther.compiler.types.ResolvedCase;
import souther.compiler.types.Type;
import souther.compiler.types.TypeSymbol;
import souther.compiler.types.ValueName;
import souther.nativecode.NotLowered;

import java.util.ArrayList;
import java.util.Collection;
import java.util.HashMap;
import java.util.LinkedHashMap;
import java.util.LinkedHashSet;
import java.util.List;
import java.util.Map;
import java.util.Set;
import java.util.StringJoiner;
import java.util.function.Function;

/**
 * A checked program written out for the half that lowers it.
 *
 * <p>This decides nothing. What it writes is the shape the checker settled, node for node, and the
 * reason it is a projection rather than a translation is that every question a lowering asks has
 * already been answered upstream — what a call reaches, what an arm binds. A writer that started
 * answering one of them would be a second place the language means something, and the two would
 * come apart. What a function value captures is not one of these: a {@link Core.Block} is written
 * out whole, params and body, and which of its free names a backend has to carry as runtime state
 * is that backend's own representation question — the same way where a field sits is the
 * lowering's and not this writer's.
 *
 * <p>So the walk over {@link Core} lists every kind there is. A kind added to the language stops
 * this compiling, which is the point: the alternative is a default arm that lets a new node cross
 * as whatever it resembles.
 *
 * <p>Two things are refused and they are refused in different places. A node whose shape on the
 * wire has not been designed cannot be written at all, and that is this writer's to say. What a
 * closed set spells — an operator, a primitive — needs no design, so all of them are written and
 * whether one can be lowered is answered by the half that lowers. Keeping a list here of what the
 * driver supports would be a second table of the same fact, and the two would come apart the first
 * time the driver learnt something this had not been told.
 *
 * <p>A binding crosses as a number and not as the compiler's own identity for it. The number is
 * this document's, counted where the binder is written, so what the far side reads is complete in
 * what it was handed.
 */
public final class ProgramWriter {

    /**
     * What the far side must agree it is reading. It moves when the meaning of something already
     * written moves, so that a driver and a writer that disagree say so rather than producing an
     * object that is wrong quietly.
     */
    public static final int TRANSPORT_VERSION = 14;

    private final CheckedProgram program;

    /**
     * What is done about each behavior's {@code ensures}, as the checker answered it for every
     * behavior of a module this compile checked.
     *
     * <p>Gathered off the modules and not asked of {@link BehaviorTarget}, which does not carry it:
     * a target is answered for a behavior of a module this compile never checked too, and what is
     * done about that one's clause is not this compile's to say ({@link #enforcement}).
     */
    private final Map<ValueName.Behavior, EnsuresEnforcement> enforcements = new HashMap<>();

    /**
     * Where a {@link Core.Block} written into the document stands, counted document-wide rather
     * than per body: two blocks nested in one body and two blocks in different bodies are told
     * apart the same way, by where each is met walking the document, so the far side can declare
     * one lifted function per site without asking this writer which body a site belongs to.
     *
     * <p>Not a closure layout. What this counts is a site's own identity on the wire, the same kind
     * of surrogate {@link Bindings} already is for a binding: the far side is where what a site's
     * function captures, and how, is decided — this only gives each site a number to declare a
     * function under before that decision is made.
     */
    private int blockSites;

    /**
     * What this walk has met, which is not what the program emits.
     *
     * <p>A body names a behavior and a type, and either may be one a module read off the path
     * declares — a module this compile did not check and does not emit. What a call can reach is
     * therefore a wider question than what is emitted, and taking one list for both answers leaves
     * a call reaching a name nothing in the document says anything about.
     */
    private final Set<ValueName.Behavior> behaviorsMet = new LinkedHashSet<>();
    private final Set<TypeSymbol.AtModule> declarationsMet = new LinkedHashSet<>();

    private ProgramWriter(CheckedProgram program) {
        this.program = program;
    }

    /** The whole program as one document. */
    public static String written(CheckedProgram program) {
        return new ProgramWriter(program).document();
    }

    /**
     * Every spelling this writer can write, for each vocabulary both halves hold a copy of.
     *
     * <p>A vocabulary the language closed is spelt twice: once here, and once by the driver that
     * reads it. Neither copy holds the other to anything. A member spelt differently on the two
     * sides is loud — the driver refuses a word it does not read, and says the two halves disagree
     * rather than that the backend is behind — but a member spelt as another member of the same
     * vocabulary is not: the document reads, and the program means something other than it says.
     *
     * <p>So this is written out to a document the driver's own test reads back, which is what the
     * addition fixture already is: the two halves meet at something one of them produced instead of
     * at two readings of the same prose. The order is the upstream enum's, and it is the
     * correspondence — the driver's test names the member it expects at each place, so a spelling
     * that moves on either side is red on the other.
     *
     * <p>Seven vocabularies and not nine. What a behavior does instead of carrying a body, and what
     * a call reaches, are switches over shapes rather than over an enum, so there is no member to
     * ask for the spelling of without an instance of one to hand. Those cross under a real program
     * or not at all.
     */
    public static String vocabularies() {
        return "{\"transport\":" + TRANSPORT_VERSION
                + ",\"op\":" + spellings(BinOp.values(), ProgramWriter::op)
                + ",\"prim\":" + spellings(Type.Prim.values(), ProgramWriter::prim)
                + ",\"publication\":" + spellings(Publication.values(), ProgramWriter::publication)
                + ",\"declaredby\":" + spellings(DeclaredBy.values(), ProgramWriter::by)
                + ",\"abort\":" + spellings(AbortKind.values(), ProgramWriter::abort)
                + ",\"leafscalar\":" + spellings(LeafScalar.values(), ProgramWriter::leaf)
                + ",\"languagecase\":" + spellings(LanguageCaseId.values(), ProgramWriter::languageCase)
                + "}";
    }

    private static <A> String spellings(A[] members, Function<A, String> spelt) {
        StringJoiner words = new StringJoiner(",", "[", "]");
        for (A member : members) {
            words.add(quoted(spelt.apply(member)));
        }
        return words.toString();
    }

    /**
     * What the object defines and what it may reach, written apart.
     *
     * <p>The bodies are walked first, because walking one is what says which behaviors and which
     * declarations the document has to carry. Each of those is then asked of the program, which is
     * the one thing that knows — a reader working a callee's signature out from the arguments at a
     * call would be rebuilding a decision out of less than it was made from.
     */
    private String document() {
        StringJoiner modules = new StringJoiner(",", "[", "]");
        for (CheckedModule module : program.modules()) {
            for (CheckedBehavior behavior : module.behaviors()) {
                behaviorsMet.add(behavior.name());
                enforcements.put(behavior.name(), behavior.ensures());
            }
            // Every declaration a module of this compile declares, whether a body here meets it
            // or not: its token and its constructor are this build's to define, and another build
            // constructing a value of it, or forking on one, reaches them here.
            for (CheckedData data : module.data()) {
                declarationsMet.add(data.name());
            }
            modules.add(module(module));
        }

        Map<ValueName.Behavior, String> behaviors = new LinkedHashMap<>();
        Map<TypeSymbol.AtModule, String> declarations = new LinkedHashMap<>();
        close(behaviors, declarations);

        return "{\"transport\":" + TRANSPORT_VERSION
                + ",\"declarations\":" + joined(declarations.values())
                + ",\"behaviors\":" + joined(behaviors.values())
                + ",\"modules\":" + modules + "}";
    }

    /**
     * Writes both tables until neither has anything left to write.
     *
     * <p>Together, because each is where the other's entries come from: writing a behavior's
     * signature names a type, writing a type's fields names more types, and one of those may be a
     * behavior's parameter that no body ever reads. Finished one at a time, whichever went first
     * would be closed over a set the second was still adding to — and which went first would be
     * the order two expressions happen to be written in rather than anything about the program.
     */
    private void close(Map<ValueName.Behavior, String> behaviors,
                       Map<TypeSymbol.AtModule, String> declarations) {
        while (true) {
            boolean grew = false;
            for (ValueName.Behavior name : new ArrayList<>(behaviorsMet)) {
                if (!behaviors.containsKey(name)) {
                    behaviors.put(name, target(name, program.behavior(name)));
                    grew = true;
                }
            }
            for (TypeSymbol.AtModule name : new ArrayList<>(declarationsMet)) {
                if (!declarations.containsKey(name)) {
                    declarations.put(name, declaration(name, program.declaration(name)));
                    grew = true;
                }
            }
            if (!grew) {
                return;
            }
        }
    }

    private String joined(Collection<String> written) {
        StringJoiner out = new StringJoiner(",", "[", "]");
        written.forEach(out::add);
        return out.toString();
    }

    /**
     * What a declared type is made of, and who declared it.
     *
     * <p>The shape and not the layout: how many fields there are and what they are called, which is
     * the declaration's own answer, while what a field costs and where it sits is the lowering's.
     * A writer that started saying where a field goes would be deciding the representation from the
     * side that never emits one.
     *
     * <p>Its module and its own name apart, because this is the one place in the document a
     * declared type's identity is owned: a value of the type says which type it is with a symbol
     * built from the two, and that symbol is what a linker resolves. Everywhere else a type is
     * named the document carries the key that reaches this — so the two halves are joined in one
     * place and split in none.
     *
     * <p>What is not written here is whether the module publishes the type. That is the module's
     * answer about its surface and not a fact about the declaration, so it crosses with the module
     * ({@link #module}).
     */
    private String declaration(TypeSymbol.AtModule name, Declared declared) {
        String identity = "{\"module\":" + quoted(name.module())
                + ",\"name\":" + quoted(name.name())
                + ",\"by\":" + quoted(by(declared.declaredBy()));
        return switch (declared.data()) {
            case CheckedData.Product it -> {
                Bindings bindings = fieldsBound(it);
                yield identity + ",\"is\":\"product\",\"fields\":" + fields(it, bindings)
                        + clauses(declared, it, bindings) + "}";
            }
            // A newtype holds one value and is told apart from a product of one field by what may
            // be written of it, which is the checker's business and settled before this. Its one
            // field is written as one: a list that happens to hold one would be a shape the reader
            // has to be told is never longer.
            case CheckedData.Newtype it -> {
                Bindings bindings = fieldsBound(it);
                yield identity + ",\"is\":\"newtype\",\"field\":"
                        + field(it.fields().getFirst(), it.codecShapes().getFirst(), bindings)
                        + clauses(declared, it, bindings) + "}";
            }
            // No field and no clause: a unit has neither, and writing an empty list of each would
            // be writing a place for them.
            case CheckedData.Unit it -> identity + ",\"is\":\"unit\"}";
            // A sum is never built, so it has no fields of its own; what it says is which types
            // stand as its cases, and a case may be a sum again.
            case CheckedData.Sum it -> {
                StringJoiner cases = new StringJoiner(",", "[", "]");
                for (TypeSymbol held : it.cases()) {
                    cases.add(caseOf(held));
                }
                yield identity + ",\"is\":\"sum\",\"cases\":" + cases
                        + ",\"form\":" + form(it.representation()) + "}";
            }
        };
    }

    /**
     * Who declared a type, as the checker answered it.
     *
     * <p>What it decides on the far side is who defines the token a value of the type is tagged by:
     * the build that checked the module is where the declaration is at home, and every other object
     * that names the type reaches that one. Written out word by word rather than taken from the name
     * the enum carries, for the reason an operator is.
     *
     * <p>A provenance added upstream stops this compiling, which is what keeps this a report of
     * the checker's answer. A writer that worked the answer out instead — by asking whether the
     * module is one this document carries — would be right about two of these three and file the
     * language's own declarations under the one they are not.
     */
    private static String by(DeclaredBy who) {
        return switch (who) {
            case A_MODULE -> "amodule";
            case A_MODULE_ON_THE_PATH -> "onthepath";
            case THE_LANGUAGE -> "thelanguage";
        };
    }

    /**
     * Every field with what it carries across the boundary, bound in one record.
     *
     * <p>Not two lists side by side: the codec shape belongs to the field and not to a position,
     * and a reader pairing two lists by index would be trusting an agreement nothing on the wire
     * holds it to. What a field carries is also what names the declarations it reaches, so a type
     * only a field holds is met here — a declaration is where a type stops being reachable from
     * anything but itself.
     */
    private String fields(CheckedData.WithFields held, Bindings bindings) {
        List<ValueShape.Field> fields = held.fields();
        List<CheckedCodecShape> codecs = held.codecShapes();
        if (fields.size() != codecs.size()) {
            throw new IllegalStateException(held.name() + " carries " + fields.size()
                    + " fields and " + codecs.size() + " codec shapes");
        }
        StringJoiner written = new StringJoiner(",", "[", "]");
        for (int at = 0; at < fields.size(); at++) {
            written.add(field(fields.get(at), codecs.get(at), bindings));
        }
        return written.toString();
    }

    /**
     * A field, what it carries, and the number a clause reads it under.
     *
     * <p>The number is the binding the checker gave the field, counted in this declaration's own
     * {@link Bindings}, and not where the field sits. A clause a spread takes in reads the binding of
     * the declaration that wrote the field, so the two are different facts that happen to agree
     * most of the time, and a reader putting a field's value under its position would be running a
     * clause over a value it was not written about the first time they did not.
     */
    private String field(ValueShape.Field field, CheckedCodecShape codec, Bindings bindings) {
        return "{\"name\":" + quoted(field.name())
                + ",\"binding\":" + bindings.of(field.binding(), field.name())
                + ",\"codec\":" + codec(codec) + "}";
    }

    /**
     * The bindings a declaration's clauses read its fields through, numbered before any clause is
     * written: a clause reads a field and binds nothing a field is.
     */
    private static Bindings fieldsBound(CheckedData.WithFields held) {
        Bindings bindings = new Bindings();
        for (ValueShape.Field field : held.fields()) {
            bindings.number(field.binding());
        }
        return bindings;
    }

    /**
     * What a value of this is held to, where this build is the one that runs it.
     *
     * <p>A declaration a module of this compile declares is built by this build's object, which
     * runs its clauses; one a module on the path declares is built by the object of the build that
     * checked it, and a construction here calls that one. So its clauses are that build's and not
     * written here: what they read and call is that build's own, a helper it keeps among it, and a
     * copy of the clauses would be run without the rest of what they were checked against.
     */
    private String clauses(Declared declared, CheckedData.WithFields held, Bindings bindings) {
        return switch (declared.declaredBy()) {
            case A_MODULE -> ",\"invariants\":" + invariants(held, bindings);
            case A_MODULE_ON_THE_PATH -> "";
            // What the language declares is a set of alternatives or a single value, and neither
            // is built from fields.
            case THE_LANGUAGE -> throw new IllegalStateException(
                    held.name() + " is declared by the language and has fields");
        };
    }

    /**
     * Every clause a value of this has to hold, in the order a failure is decided in: the name it
     * is reported under, where the author gave one, and the condition as the checker elaborated
     * it over the fields' bindings.
     *
     * <p>Every clause that applies and not the ones this declaration wrote, since a spread carries
     * the clauses of what it takes in, and the checker has already said which those are.
     */
    private String invariants(CheckedData.WithFields held, Bindings bindings) {
        StringJoiner written = new StringJoiner(",", "[", "]");
        for (ValueShape.Invariant clause : held.invariants()) {
            String name = clause.name().map(ProgramWriter::quoted).orElse("null");
            written.add("{\"name\":" + name
                    + ",\"condition\":" + core(clause.condition(), bindings) + "}");
        }
        return written.toString();
    }

    /** What a field carries across the boundary, as the check derived it. */
    private String codec(CheckedCodecShape shape) {
        return switch (shape) {
            case CheckedCodecShape.Scalar it ->
                    "{\"is\":\"scalar\",\"scalar\":" + quoted(leaf(it.kind())) + "}";
            case CheckedCodecShape.Named it ->
                    "{\"is\":\"named\",\"declared\":" + quoted(declaredName(it.name())) + "}";
            case CheckedCodecShape.ListOf it ->
                    "{\"is\":\"listof\",\"element\":" + codec(it.element()) + "}";
            case CheckedCodecShape.SetOf it ->
                    "{\"is\":\"setof\",\"element\":" + codec(it.element()) + "}";
            case CheckedCodecShape.MapOf it -> "{\"is\":\"mapof\",\"key\":" + key(it.key())
                    + ",\"value\":" + codec(it.value()) + "}";
            case CheckedCodecShape.OptionOf it ->
                    "{\"is\":\"optionof\",\"present\":" + codec(it.present()) + "}";
        };
    }

    /** What a parameter can arrive as. */
    private String input(CheckedBoundaryInput shape) {
        return switch (shape) {
            case CheckedBoundaryInput.Scalar it ->
                    "{\"is\":\"scalar\",\"scalar\":" + quoted(leaf(it.scalar())) + "}";
            case CheckedBoundaryInput.Nominal it ->
                    "{\"is\":\"nominal\",\"declared\":" + quoted(declaredName(it.name())) + "}";
            case CheckedBoundaryInput.ListOf it ->
                    "{\"is\":\"listof\",\"element\":" + input(it.element()) + "}";
            case CheckedBoundaryInput.SetOf it ->
                    "{\"is\":\"setof\",\"element\":" + input(it.element()) + "}";
            case CheckedBoundaryInput.MapOf it -> "{\"is\":\"mapof\",\"key\":" + key(it.key())
                    + ",\"value\":" + input(it.value()) + "}";
        };
    }

    /**
     * What an answer can leave as.
     *
     * <p>A union answer carries its type as written beside the cases the boundary descended to,
     * because the two are different answers: rebuilding the type from the cases would give a union
     * nobody wrote.
     */
    private String output(CheckedBoundaryOutput shape) {
        return switch (shape) {
            case CheckedBoundaryOutput.Scalar it ->
                    "{\"is\":\"scalar\",\"scalar\":" + quoted(leaf(it.scalar())) + "}";
            case CheckedBoundaryOutput.Nominal it ->
                    "{\"is\":\"nominal\",\"declared\":" + quoted(declaredName(it.name())) + "}";
            case CheckedBoundaryOutput.ListOf it ->
                    "{\"is\":\"listof\",\"element\":" + output(it.element()) + "}";
            case CheckedBoundaryOutput.SetOf it ->
                    "{\"is\":\"setof\",\"element\":" + output(it.element()) + "}";
            case CheckedBoundaryOutput.MapOf it -> "{\"is\":\"mapof\",\"key\":" + key(it.key())
                    + ",\"value\":" + output(it.value()) + "}";
            case CheckedBoundaryOutput.Cases it -> {
                StringJoiner cases = new StringJoiner(",", "[", "]");
                for (TypeSymbol held : it.cases()) {
                    cases.add(caseOf(held));
                }
                yield "{\"is\":\"cases\",\"type\":" + type(it.type()) + ",\"cases\":" + cases
                        + ",\"form\":" + form(it.representation()) + "}";
            }
        };
    }

    /** What a boundary map's key is written as. */
    private String key(MapKeyRepresentation key) {
        return switch (key) {
            case MapKeyRepresentation.Text it -> "{\"is\":\"text\"}";
            case MapKeyRepresentation.Date it -> "{\"is\":\"date\"}";
            case MapKeyRepresentation.Time it -> "{\"is\":\"time\"}";
            case MapKeyRepresentation.DateTime it -> "{\"is\":\"datetime\"}";
            case MapKeyRepresentation.Instant it -> "{\"is\":\"instant\"}";
            case MapKeyRepresentation.NamedKey it ->
                    "{\"is\":\"namedkey\",\"declared\":" + quoted(declaredName(it.name())) + "}";
        };
    }

    /** How a set of alternatives travels, with both of its keys where it has them. */
    private static String form(CheckedAlternativesForm form) {
        return switch (form) {
            case CheckedAlternativesForm.Enumeration it -> "{\"is\":\"enumeration\"}";
            case CheckedAlternativesForm.Discriminated it -> "{\"is\":\"discriminated\",\"tag\":"
                    + quoted(it.tagKey()) + ",\"contents\":" + quoted(it.contentsKey()) + "}";
        };
    }

    private static String leaf(LeafScalar leaf) {
        return switch (leaf) {
            case STRING -> "STRING";
            case INT -> "INT";
            case BOOL -> "BOOL";
            case DECIMAL -> "DECIMAL";
            case DATE -> "DATE";
            case TIME -> "TIME";
            case DATETIME -> "DATETIME";
            case INSTANT -> "INSTANT";
        };
    }

    private static String languageCase(LanguageCaseId id) {
        return switch (id) {
            case SOME -> "SOME";
            case NONE -> "NONE";
            case DIVISION_BY_ZERO -> "DIVISION_BY_ZERO";
            case NOT_A_NUMBER -> "NOT_A_NUMBER";
            case NOT_A_DATE -> "NOT_A_DATE";
            case NOT_A_TIME -> "NOT_A_TIME";
            case NOT_WHOLE -> "NOT_WHOLE";
            case NOT_A_FINITE_DECIMAL -> "NOT_A_FINITE_DECIMAL";
        };
    }

    private String module(CheckedModule module) {
        StringJoiner definitions = new StringJoiner(",", "[", "]");
        for (CheckedBehavior behavior : module.behaviors()) {
            switch (behavior.implementation()) {
                case CheckedImplementation.Body written -> definitions.add(body(module, behavior, written));
                case CheckedImplementation.Composed written -> definitions.add(composed(module, behavior, written));
                // Named in the table of targets and not defined by this object: what answers it
                // is supplied from outside, implemented by another build, or not written at all,
                // and none of those is a definition this module holds.
                case CheckedImplementation.Injected ignored -> { }
                case CheckedImplementation.ImplementedElsewhere ignored -> { }
                case CheckedImplementation.Unwritten ignored -> { }
            }
        }
        StringJoiner helpers = new StringJoiner(",", "[", "]");
        for (CheckedHelper helper : module.helpers()) {
            helpers.add(helper(helper));
        }
        StringJoiner values = new StringJoiner(",", "[", "]");
        for (CheckedValue value : module.values()) {
            values.add(value(value));
        }
        StringJoiner entries = new StringJoiner(",", "[", "]");
        for (CheckedValueEntry entry : module.valueEntries()) {
            entries.add(entry(entry));
        }
        StringJoiner examples = new StringJoiner(",", "[", "]");
        for (CheckedBehavior behavior : module.behaviors()) {
            List<CheckedRow> rows = behavior.rows();
            for (int at = 0; at < rows.size(); at++) {
                String entry = example(module, behavior, at, rows.get(at));
                if (entry != null) {
                    examples.add(entry);
                }
            }
        }
        // What the module publishes of the data it declares, which is its surface and not a fact
        // about any one declaration: what another build can name, and so build a value of.
        StringJoiner publishes = new StringJoiner(",", "[", "]");
        for (CheckedData data : module.data()) {
            if (module.publicationOf(data.name()) == Publication.PUBLISHED) {
                publishes.add(quoted(named(data.name())));
            }
        }
        return "{\"name\":" + quoted(module.name())
                + ",\"publishes\":" + publishes
                + ",\"helpers\":" + helpers
                + ",\"values\":" + values
                + ",\"entries\":" + entries
                + ",\"definitions\":" + definitions
                + ",\"examples\":" + examples + "}";
    }

    /**
     * An entry that runs one of a behavior's rows, or nothing where the row states no values.
     *
     * <p>Numbered by where the row stands among the behavior's rows and not by how many entries
     * have been written, so a row the compile could not read leaves its number unused rather than
     * moving every row after it onto a number that was somebody else's.
     *
     * <p>A row the compile could not read is the one case with no entry, and it is the one case
     * where there is nothing to run: the values it would hand over were never read. Every other
     * row is written, and a value this writer cannot make an expression for refuses the program
     * the same way a body it cannot write does — the language admits the program and this backend
     * does not write it yet.
     */
    private String example(CheckedModule module, CheckedBehavior behavior, int at, CheckedRow row) {
        RowStatement.Stated states = switch (row.statement()) {
            case CheckedRow.SelfContained it -> it.states();
            case CheckedRow.WithStandIns it -> it.states();
            // Its answer is owed, which is a row nothing holds an answer to and still a row whose
            // values were read. The entry runs it; what the run answers is nobody's claim yet.
            case CheckedRow.AnswerOwed it -> it.states();
            case CheckedRow.NotReproducible it -> null;
        };
        if (states == null) {
            return null;
        }
        return "{\"behavior\":" + quoted(behavior.name().name())
                + ",\"at\":" + at
                + ",\"body\":" + applied(module, behavior, states) + "}";
    }

    /**
     * The one call a row is: the behavior, handed the values the row states.
     *
     * <p>A call and not a shape of its own, so that what an entry does is lowered by whatever
     * lowers a call and the two cannot come apart. What the behavior answers is read off its
     * signature, which is also where the arguments' types come from — a row states values and the
     * signature is what says at which type each of them is handed over.
     */
    private String applied(CheckedModule module, CheckedBehavior behavior,
                           RowStatement.Stated states) {
        List<Type> takes = behavior.signature().takes();
        List<ObservedValue> inputs = states.inputs();
        if (inputs.size() != takes.size()) {
            throw notYet("a row of `" + behavior.name() + "` stating " + inputs.size()
                    + " values where the behavior takes " + takes.size());
        }
        StringJoiner arguments = new StringJoiner(",", "[", "]");
        for (int at = 0; at < takes.size(); at++) {
            arguments.add(given(inputs.get(at), takes.get(at)));
        }
        // No Core.Call stands behind this one for program.abortsAt to ask of, and none is needed:
        // what AbortSites answers for a call reaching a behavior is NONE, whatever the behavior is.
        // What the behavior ends with is its own, and what its clause ends with is what the
        // target's ensures says (EnsuresEnforcement#aborts), not a fact of this site — the same
        // answer a call written in a body gets.
        return "{\"core\":\"call\",\"reaches\":{\"is\":\"behavior\",\"declared\":"
                + quoted(module.name() + "." + behavior.name().name()) + "}"
                + ",\"arguments\":" + arguments
                + ",\"type\":" + type(behavior.signature().answers())
                + ",\"aborts\":[]}";
    }

    /**
     * A value a row states, written as the expression that makes it.
     *
     * <p>At the type it is handed over at as well as by what it is, because the value does not say
     * on its own: a number handed to a parameter of a type that holds one is a different expression
     * from the same number handed to an {@code Int}, and a writer reading only the value would
     * write the second where the first was meant.
     *
     * <p>Every kind of value a row can state is answered for, and the ones with no expression here
     * say so. Caught by an arm standing for the rest, a value a row can state would cross as
     * whatever it resembled.
     */
    private String given(ObservedValue value, Type at) {
        // A literal, the same as elsewhere: nothing to ask program.abortsAt of, and NONE for the
        // same reason applied() states it — a literal never aborts, whatever site holds it.
        return switch (value) {
            case ObservedValue.Integer it when at == Type.Prim.INT ->
                    "{\"core\":\"int\",\"value\":" + it.value() + ",\"type\":" + type(at)
                            + ",\"aborts\":[]}";
            case ObservedValue.Bool it when at == Type.Prim.BOOL ->
                    "{\"core\":\"bool\",\"value\":" + it.value() + ",\"type\":" + type(at)
                            + ",\"aborts\":[]}";
            case ObservedValue.Text it when at == Type.Prim.STRING ->
                    "{\"core\":\"string\",\"value\":" + quoted(it.value())
                            + ",\"type\":" + type(at) + ",\"aborts\":[]}";

            case ObservedValue.Integer it -> throw notStated(it, at);
            case ObservedValue.Bool it -> throw notStated(it, at);
            case ObservedValue.Decimal it -> throw notStated(it, at);
            case ObservedValue.Text it -> throw notStated(it, at);
            case ObservedValue.Temporal it -> throw notStated(it, at);
            case ObservedValue.Unit it -> throw notStated(it, at);
            case ObservedValue.Constructed it -> throw notStated(it, at);
            case ObservedValue.Sequence it -> throw notStated(it, at);
            case ObservedValue.Mapping it -> throw notStated(it, at);
            case ObservedValue.Absent it -> throw notStated(it, at);
            case ObservedValue.Unknown it -> throw notStated(it, at);
            case ObservedValue.Truncated it -> throw notStated(it, at);
        };
    }

    private static NotLowered notStated(ObservedValue value, Type at) {
        return new NotLowered("a row stating " + value + " at " + at);
    }

    /**
     * A definition the module holds as one of its own.
     *
     * <p>A module carries every helper it reaches, including one another module declares, so what
     * a helper is called here is where it is declared and what it is holding is which module is
     * holding it. Two modules holding one helper hold a copy each, which is what the language says
     * a published helper is.
     *
     * <p>Each parameter is written once, its name and its type together, because they are one
     * {@link CheckedHelper.Parameter}; written as two lists they would be two statements of how many
     * there are. What the helper answers is not written at all: it is its body's type, which the body
     * already carries, and a second copy would only be something a reader has to hold to the first.
     */
    private String helper(CheckedHelper helper) {
        Bindings bindings = new Bindings();
        StringJoiner parameters = new StringJoiner(",", "[", "]");
        for (CheckedHelper.Parameter parameter : helper.parameters()) {
            bindings.number(parameter.binder().binding());
            parameters.add("{\"name\":" + quoted(parameter.binder().name())
                    + ",\"type\":" + type(parameter.type()) + "}");
        }
        return "{\"declared\":" + quoted(reached(helper.declares()))
                + ",\"parameters\":" + parameters
                + ",\"body\":" + core(helper.body(), bindings)
                + "}";
    }

    /**
     * A value this module declares: the one place it runs.
     *
     * <p>Its identity crosses split, module and name apart, the way a behavior's does
     * ({@link Target}) and a helper's does not: {@code value_symbol(module, name)} is built from the
     * two, and a joined spelling would have to be split back up to get there.
     *
     * <p>What its method is handed is not a parameter but a handover: another value this one's root
     * region names, built already and passed rather than rebuilt. A reader wanting what a call to
     * this value's home has to supply reads {@code handovers}, never {@code parameters.isEmpty()} —
     * a value takes none.
     *
     * <p>Not written with a {@code publication} of its own. {@link CheckedModule}'s constructor
     * already holds "published ⇔ has a {@link CheckedValueEntry} among {@link
     * CheckedModule#valueEntries()}" as an invariant, so a field here would be the same fact
     * written twice — and a reader wanting to know would ask {@link CheckedModule#valueEntries()}
     * or {@link CheckedModule#publicationOfValue}, never this.
     *
     * <p>Nor with what it answers: that is {@link CheckedValue#answers()}, which is its body's type,
     * and the body carries it already.
     */
    private String value(CheckedValue value) {
        Bindings bindings = new Bindings();
        StringJoiner handovers = new StringJoiner(",", "[", "]");
        for (CheckedValue.Handover handover : value.handovers()) {
            bindings.number(handover.binder().binding());
            handovers.add("{\"parameter\":" + quoted(handover.binder().name())
                    + ",\"type\":" + type(handover.type())
                    + ",\"carries\":{\"module\":" + quoted(handover.carries().module())
                    + ",\"name\":" + quoted(handover.carries().name()) + "}}");
        }
        return "{\"module\":" + quoted(value.name().module())
                + ",\"name\":" + quoted(value.name().name())
                + ",\"handovers\":" + handovers
                + ",\"body\":" + core(value.body(), bindings)
                + "}";
    }

    /**
     * The entry this module publishes for a value: the nullary bridge another module calls in place
     * of holding a copy of the value (ADR-0074).
     *
     * <p>{@code body} is a reference to the value and nothing else — a call reaching
     * {@link Core.Reached.OfValue}, written the same way any other reach to the value is, and never
     * a fresh node kind this writer has to design for. Not written with {@code publication}: every
     * entry this module holds is for a value it publishes, which
     * {@link CheckedModule#valueEntries()}'s own invariant already guarantees, so a reader has
     * nothing to ask here that {@code module.publicationOfValue(entry.value())} would not answer
     * {@code PUBLISHED} to unconditionally.
     */
    private String entry(CheckedValueEntry entry) {
        Bindings bindings = new Bindings();
        return "{\"value\":{\"module\":" + quoted(entry.value().module())
                + ",\"name\":" + quoted(entry.value().name()) + "}"
                + ",\"body\":" + core(entry.body(), bindings)
                + "}";
    }

    /** How a definition of a module is named on the wire: its module, then its own name. */
    private String reached(ValueName name) {
        return switch (name) {
            case ValueName.OfAModule it -> it.module() + "." + it.name();
            // A name that is in scope where it stands, and one the standard library declares.
            // Neither is a definition a module holds, and nothing here reaches one.
            case ValueName.InScope it -> throw notYet("a call reaching " + it.name());
            case ValueName.Stdlib it -> throw notYet("a call reaching " + it.name());
        };
    }

    /**
     * How a declared type is referred to on the wire.
     *
     * <p>Its module and then its own name. A module's name carries dots and a type's carries none,
     * so the last segment is the type and no two declarations are spelt the same way.
     *
     * <p>A key and not an identity. What the key reaches is the declaration, which carries the
     * module and the name apart; this is the one place the two are joined, and neither half of the
     * compiler splits one back up. Written the other way round, every reader of a type name would
     * be recovering an identity from a spelling — which is what the same convention already says
     * a behavior's identity must not be.
     */
    private String named(TypeSymbol.AtModule name) {
        declarationsMet.add(name);
        return name.module() + "." + name.name();
    }

    /**
     * The same for a name standing where only a declaration can, which is refused rather than
     * guessed at: what is built, what a field or a key is named by, what a boundary names.
     */
    private String declaredName(TypeSymbol name) {
        return switch (name) {
            case TypeSymbol.AtModule it -> named(it);
            case TypeSymbol.Primitive it ->
                    throw notYet("the primitive " + it.name() + " named as a declaration");
            case TypeSymbol.LanguageCase it ->
                    throw notYet("the case " + it.name() + " named as a declaration");
        };
    }

    /**
     * Which case a name is, where a case may be any of the three the language has: one a module
     * declares, a primitive standing as a case, or one the language gives.
     *
     * <p>The identity and not the shape. How a case is written at a boundary is read on the far
     * side off what the identity reaches — a declaration's arm, or a primitive's being one — so
     * nothing about the shape crosses here that the identity does not already answer.
     */
    private String caseOf(TypeSymbol name) {
        return switch (name) {
            case TypeSymbol.AtModule it ->
                    "{\"is\":\"declared\",\"declared\":" + quoted(named(it)) + "}";
            case TypeSymbol.Primitive it ->
                    "{\"is\":\"primitive\",\"prim\":" + quoted(prim(it.primitive())) + "}";
            case TypeSymbol.LanguageCase it ->
                    "{\"is\":\"language\",\"case\":" + quoted(languageCase(it.id())) + "}";
        };
    }

    /**
     * A behavior, and how it comes to answer.
     *
     * <p>A body is emitted. The rest are ways of answering that are not code this program holds —
     * supplied by the caller, implemented by another build, composed out of other behaviors, or
     * not written at all — and which of them it is crosses, because it decides what the object
     * says about the name rather than what it puts under it.
     */
    private String target(ValueName.Behavior name, BehaviorTarget behavior) {
        String how = switch (behavior.implementation()) {
            case CheckedImplementation.Body it -> "body";
            case CheckedImplementation.Injected it -> "injected";
            case CheckedImplementation.ImplementedElsewhere it -> "elsewhere";
            case CheckedImplementation.Composed it -> "composed";
            case CheckedImplementation.Unwritten it -> "unwritten";
        };
        StringJoiner inputs = new StringJoiner(",", "[", "]");
        for (CheckedBoundaryInput input : behavior.signature().inputs()) {
            inputs.add(input(input));
        }
        // The module and the name apart, which is what an identity is made of and what a symbol is
        // built from. Joined into one string it would have to be split back, and a module's name
        // carries dots.
        return "{\"module\":" + quoted(name.module())
                + ",\"name\":" + quoted(name.name())
                + ",\"is\":" + quoted(how)
                + ",\"inputs\":" + inputs
                + ",\"output\":" + output(behavior.signature().output())
                + ",\"ensures\":" + ensures(enforcement(name))
                + "}";
    }

    /**
     * What is done about a behavior's {@code ensures}, as the checker answered it for a behavior of
     * a module this compile checked, and {@link EnsuresEnforcement.NotDecidedHere} for one of a
     * module it did not — which is the reading {@link EnsuresEnforcement#in} gives a miss.
     *
     * <p>A miss for a behavior of a module this compile checked is not an answer: every behavior of
     * such a module was walked above, so it is this writer having missed one, and it stops.
     */
    private EnsuresEnforcement enforcement(ValueName.Behavior name) {
        EnsuresEnforcement decided = enforcements.get(name);
        if (decided != null) {
            return decided;
        }
        for (CheckedModule module : program.modules()) {
            if (module.name().equals(name.module())) {
                throw new IllegalStateException("no enforcement decision for `" + name.name()
                        + "`, which `" + module.name() + "` declares");
            }
        }
        return EnsuresEnforcement.NotDecidedHere.INSTANCE;
    }

    /**
     * Where a behavior's answer is held to what it declares, with the rules that say what that is.
     *
     * <p>The four answers the checker has, each as itself. Two of them carry the rules and say where
     * they run; the other two carry none, and they are not one answer: {@code none} is a behavior
     * that was read and declares nothing, {@code undecided} one whose clause this compile does not
     * run because nobody here decided where it would be run.
     */
    private String ensures(EnsuresEnforcement enforcement) {
        return switch (enforcement) {
            case EnsuresEnforcement.AtTheCallee it ->
                    "{\"at\":\"callee\",\"contract\":" + contract(it.contract()) + "}";
            case EnsuresEnforcement.AtEachCrossing it ->
                    "{\"at\":\"crossing\",\"contract\":" + contract(it.contract()) + "}";
            case EnsuresEnforcement.NoContract it -> "{\"at\":\"none\"}";
            case EnsuresEnforcement.NotDecidedHere it -> "{\"at\":\"undecided\"}";
        };
    }

    /**
     * The rules a behavior's answer is held to, over the parameters' bindings and the answer's.
     *
     * <p>The parameters are numbered first and in the order the signature takes them, the way a
     * body's are, so a rule reads a parameter under the number of where it stands. What each one
     * holds is the target's inputs and is not written a second time; its name is written, as a
     * body's parameters are, so the two halves can agree on how many there are.
     *
     * <p>What the contract answers and who declared it are the target's too, and the rules are
     * written in the order the checker keeps them, which is the order a failure is decided in.
     */
    private String contract(Contract contract) {
        Bindings bindings = new Bindings();
        StringJoiner parameters = new StringJoiner(",", "[", "]");
        for (Contract.Param parameter : contract.params()) {
            bindings.number(parameter.binding());
            parameters.add(quoted(parameter.name()));
        }
        StringJoiner rules = new StringJoiner(",", "[", "]");
        for (Contract.Rule rule : contract.rules()) {
            rules.add(rule(rule, bindings));
        }
        return "{\"parameters\":" + parameters + ",\"rules\":" + rules + "}";
    }

    /**
     * One rule: which answers it applies to, the binding the answer is read through there, what has
     * to hold, and what the checker said of it beside that.
     *
     * <p>A rule over a case says what the answer is read as where it is that case, because what is
     * tested does not say it: a case that is a sum is tested as the leaves it descends to, and read
     * as the sum. A rule over every answer reads it as what the behavior answers, which the target
     * says already.
     *
     * <p>Whether the rule reads the answer, and the clause it is reported under, are carried though
     * nothing here runs either: both are what the checker decided about the declaration, and a
     * reader that later needs one would otherwise need the document to say more than it did.
     */
    private String rule(Contract.Rule rule, Bindings bindings) {
        String guard = switch (rule.guard()) {
            case Contract.Guard.Always it -> "{\"is\":\"always\"}";
            case Contract.Guard.Case it -> "{\"is\":\"case\",\"selects\":" + selects(it.selected())
                    + ",\"binds\":" + type(it.selected().bound()) + "}";
        };
        int value = bindings.number(rule.value());
        return "{\"guard\":" + guard
                + ",\"value\":" + value
                + ",\"condition\":" + core(rule.condition(), bindings)
                + ",\"readsanswer\":" + rule.readsAnswer()
                + ",\"clause\":" + rule.clause().map(ProgramWriter::quoted).orElse("null")
                + "}";
    }

    /**
     * The body of a behavior this object emits, under the name the table above knows it by.
     *
     * <p>What the module says about the name crosses with the body, which is the one place it is
     * this compile's to answer: the module declaring it is one this compile checked. What an
     * artifact then makes reachable is a different question and the artifact's own, and a body is
     * where the two meet.
     */
    private String body(CheckedModule module, CheckedBehavior behavior,
                        CheckedImplementation.Body written) {
        Bindings bindings = new Bindings();
        StringJoiner parameters = new StringJoiner(",", "[", "]");
        for (Core.Binder parameter : written.parameters()) {
            bindings.number(parameter.binding());
            parameters.add(quoted(parameter.name()));
        }
        return "{\"is\":\"body\",\"declared\":" + quoted(module.name() + "." + behavior.name().name())
                + ",\"parameters\":" + parameters
                + ",\"publication\":" + quoted(publication(module.publicationOf(behavior.name())))
                + ",\"body\":" + core(written.body(), bindings)
                + "}";
    }

    /**
     * A behavior written as {@code >->}: the stages, and what each is offered.
     *
     * <p>What {@link Composition} carries is the decision and not a plan for carrying it out, and
     * that is exactly what crosses — stage for stage, in the order the checker walked them. A
     * lowering that turned this into a pseudo-{@link Core} of calls and matches first would be a
     * second place the routing was decided, which is the mistake this composition exists to rule
     * out.
     *
     * <p>Registered the way a body's calls are: a stage's behavior may be one nothing in this
     * document's bodies ever calls — a composition's own signature is enough to name it — so it is
     * met here or nowhere, the same as a body's {@link Core.Call} meets what it reaches.
     *
     * <p>What the composition answers is not written here, and neither is what each stage answers:
     * both are the answers of behaviors the table of targets already carries, the composition's own
     * and each stage's, and a copy beside them would be a second statement to hold to the first.
     */
    private String composed(CheckedModule module, CheckedBehavior behavior,
                            CheckedImplementation.Composed written) {
        Composition composition = written.composition();
        StringJoiner stages = new StringJoiner(",", "[", "]");
        for (Composition.Stage stage : composition.stages()) {
            stages.add(stage(stage));
        }
        return "{\"is\":\"composed\",\"declared\":"
                + quoted(module.name() + "." + behavior.name().name())
                + ",\"publication\":" + quoted(publication(module.publicationOf(behavior.name())))
                + ",\"stages\":" + stages
                + "}";
    }

    /** One stage of a composition: the behavior it applies, and when. */
    private String stage(Composition.Stage stage) {
        behaviorsMet.add(stage.behavior());
        return "{\"behavior\":" + quoted(reached(stage.behavior()))
                + ",\"routing\":" + routing(stage.routing())
                + "}";
    }

    /**
     * When a stage is applied to the running value.
     *
     * <p>The cases a routing tests are declarations, the same as an arm's, so what a case
     * resolves to is met the way {@link #selects} meets one — which is what lets the routing
     * closure a stage adds into {@link #declarationsMet} reach the same {@code tag_of} a
     * {@code match} arm already resolves on the far side.
     */
    private String routing(Composition.Routing routing) {
        return switch (routing) {
            case Composition.Routing.Always ignored -> "{\"is\":\"always\"}";
            case Composition.Routing.OnCases it -> {
                StringJoiner accepted = new StringJoiner(",", "[", "]");
                for (TypeSymbol type : it.accepted()) {
                    accepted.add(caseOf(type));
                }
                yield "{\"is\":\"oncases\",\"accepted\":" + accepted + "}";
            }
        };
    }

    /** How the module's answer about a name is spelt on the wire, member by member. */
    private static String publication(Publication published) {
        return switch (published) {
            case PUBLISHED -> "published";
            case KEPT -> "kept";
        };
    }

    /**
     * The numbers this document knows a behavior's bindings by.
     *
     * <p>Counted where a binder is written, so a read of one that was never written is a read of
     * something outside what crossed, and it is refused rather than numbered here.
     */
    private static final class Bindings {

        private final Map<BindingId, Integer> numbered = new HashMap<>();

        private int counted;

        int number(BindingId binding) {
            Integer already = numbered.get(binding);
            if (already != null) {
                return already;
            }
            numbered.put(binding, counted);
            return counted++;
        }

        int of(BindingId binding, String name) {
            Integer number = numbered.get(binding);
            if (number == null) {
                throw new NotLowered("a read of a binding this document does not carry: " + name);
            }
            return number;
        }
    }

    private String core(Core node, Bindings bindings) {
        return switch (node) {
            case Core.Int it -> "{\"core\":\"int\",\"value\":" + it.value()
                    + ",\"type\":" + type(it.type()) + ",\"aborts\":" + aborts(it) + "}";
            case Core.Read it -> "{\"core\":\"read\",\"binding\":"
                    + bindings.of(it.binding(), it.name())
                    + ",\"type\":" + type(it.type()) + ",\"aborts\":" + aborts(it) + "}";
            case Core.Bool it -> "{\"core\":\"bool\",\"value\":" + it.value()
                    + ",\"type\":" + type(it.type()) + ",\"aborts\":" + aborts(it) + "}";
            // The text as the compiler read it, which is the text normalized to NFC. Nothing here
            // folds it a second time: where text arrives from outside is where that is done, and a
            // source file is one of the two places it arrives.
            case Core.Str it -> "{\"core\":\"string\",\"value\":" + quoted(it.value())
                    + ",\"type\":" + type(it.type()) + ",\"aborts\":" + aborts(it) + "}";
            case Core.Binary it -> "{\"core\":\"binary\",\"op\":" + quoted(op(it.op()))
                    + ",\"reading\":" + reading(it.reading())
                    + ",\"left\":" + core(it.left(), bindings)
                    + ",\"right\":" + core(it.right(), bindings)
                    + ",\"type\":" + type(it.type()) + ",\"aborts\":" + aborts(it) + "}";
            case Core.UnitValue it -> "{\"core\":\"unit\",\"declared\":"
                    + quoted(declaredName(it.data())) + ",\"type\":" + type(it.type())
                    + ",\"aborts\":" + aborts(it) + "}";
            case Core.Construct it -> {
                StringJoiner values = new StringJoiner(",", "[", "]");
                for (Core.FieldValue field : it.values()) {
                    values.add(core(field.value(), bindings));
                }
                yield "{\"core\":\"construct\",\"declared\":" + quoted(named(it.typeName()))
                        + ",\"values\":" + values + ",\"type\":" + type(it.type())
                        + ",\"aborts\":" + aborts(it) + "}";
            }
            case Core.FieldAccess it -> "{\"core\":\"field\",\"target\":"
                    + core(it.target(), bindings) + ",\"field\":" + quoted(it.field())
                    + ",\"type\":" + type(it.type()) + ",\"aborts\":" + aborts(it) + "}";
            case Core.Match it -> match(it, bindings);
            case Core.OptionSome it -> "{\"core\":\"some\",\"value\":" + core(it.value(), bindings)
                    + ",\"type\":" + type(it.type()) + ",\"aborts\":" + aborts(it) + "}";
            case Core.OptionNone it -> "{\"core\":\"none\",\"type\":" + type(it.type())
                    + ",\"aborts\":" + aborts(it) + "}";
            case Core.Tuple it -> {
                StringJoiner members = new StringJoiner(",", "[", "]");
                for (Core element : it.elements()) {
                    members.add(core(element, bindings));
                }
                yield "{\"core\":\"tuple\",\"members\":" + members
                        + ",\"type\":" + type(it.type()) + ",\"aborts\":" + aborts(it) + "}";
            }
            case Core.TupleGet it -> "{\"core\":\"member\",\"tuple\":" + core(it.tuple(), bindings)
                    + ",\"at\":" + it.index() + ",\"type\":" + type(it.type())
                    + ",\"aborts\":" + aborts(it) + "}";
            case Core.Neg it -> "{\"core\":\"neg\",\"operand\":" + core(it.operand(), bindings)
                    + ",\"type\":" + type(it.type()) + ",\"aborts\":" + aborts(it) + "}";
            case Core.LetIn it -> letIn(it, bindings);
            // Where the checker let a value stand as a type other than its own, which it decided
            // and this writes: what is evaluated, and the type the position takes it as.
            case Core.Widen it -> "{\"core\":\"widen\",\"value\":" + core(it.value(), bindings)
                    + ",\"type\":" + type(it.type()) + ",\"aborts\":" + aborts(it) + "}";
            case Core.If it -> "{\"core\":\"if\",\"cond\":" + core(it.cond(), bindings)
                    + ",\"then\":" + core(it.then(), bindings)
                    + ",\"else\":" + core(it.els(), bindings)
                    + ",\"type\":" + type(it.type()) + ",\"aborts\":" + aborts(it) + "}";

            case Core.Decimal it -> throw notYet("a decimal literal", it);
            case Core.Temporal it -> throw notYet("a temporal literal", it);
            case Core.MaterialisedValue it -> throw notYet("a value read from its module", it);
            case Core.Call it -> call(it, bindings);
            case Core.PreservedCall it -> throw notYet("a call kept for what it says", it);
            case Core.Apply it -> apply(it, bindings);
            case Core.IfConstructed it -> throw notYet("an attempted construction", it);
            case Core.Block it -> block(it, bindings);
            case Core.ListLit it -> throw notYet("a list", it);
            case Core.Unreachable it -> throw notYet("an unreachable", it);
        };
    }

    /**
     * A binding and what is written under it.
     *
     * <p>The value is written before the binder is numbered, because what the value names is what
     * was in scope where it stands and a binding cannot be read in its own value. Written the other
     * way round, a value mentioning a name the binder shadows would cross as a read of the binder
     * it is still being computed for.
     *
     * <p>What the binding is in force at is written beside it, and not left to the value's type or to
     * a read of the binder: the two can differ (an annotation, or a sum the value is one case of), and
     * every read of the binding is typed by this and not by the value.
     */
    private String letIn(Core.LetIn it, Bindings bindings) {
        String value = core(it.value(), bindings);
        int number = bindings.number(it.binder().binding());
        return "{\"core\":\"let\",\"binding\":" + number
                + ",\"binds\":" + type(it.bindType())
                + ",\"value\":" + value
                + ",\"body\":" + core(it.body(), bindings)
                + ",\"type\":" + type(it.type()) + ",\"aborts\":" + aborts(it) + "}";
    }

    /**
     * A function value applied to arguments: {@link Core.Apply}, and not {@link Core.Call}, because
     * what is applied is a value the body holds rather than something declared elsewhere.
     *
     * <p>{@code function} crosses as whatever {@link Core.Apply#fn()} is — a {@link Core.Read}, the
     * one thing it is ever built over — written the same way any other read is, rather than reduced
     * to the binding number alone. A reader wanting the number still finds it, at
     * {@code function.binding}, but the shape stays the one every other node's operand already has,
     * so a wider callee this compiler admits later crosses without this method changing.
     */
    private String apply(Core.Apply it, Bindings bindings) {
        StringJoiner arguments = new StringJoiner(",", "[", "]");
        for (Core argument : it.args()) {
            arguments.add(core(argument, bindings));
        }
        return "{\"core\":\"apply\",\"function\":" + core(it.fn(), bindings)
                + ",\"arguments\":" + arguments
                + ",\"type\":" + type(it.type()) + ",\"aborts\":" + aborts(it) + "}";
    }

    /**
     * A function value: its own parameters, and the body they are bound in.
     *
     * <p>Written whole and not closure-converted here. What of the body's free names a backend has
     * to carry forward as runtime state, and how, is a representation question and this writer
     * answers none of those — the same boundary that keeps where a field sits out of a declaration.
     * {@code site} is this document's own number for where the block stands ({@link #blockSites}),
     * minted so the far side has something to declare a lifted function under before it has decided
     * anything about what that function closes over.
     *
     * <p>A parameter is numbered the same way any other binder is, through the {@code bindings}
     * this body already carries — a block's parameters are read inside its own body exactly as a
     * helper's or a {@code let}'s are, and nothing about being a function value's own parameter
     * changes what crossing one means.
     */
    private String block(Core.Block it, Bindings bindings) {
        int site = blockSites++;
        StringJoiner parameters = new StringJoiner(",", "[", "]");
        for (Core.Binder parameter : it.params()) {
            int number = bindings.number(parameter.binding());
            parameters.add("{\"binding\":" + number + ",\"name\":" + quoted(parameter.name()) + "}");
        }
        return "{\"core\":\"block\",\"site\":" + site
                + ",\"parameters\":" + parameters
                + ",\"body\":" + core(it.body(), bindings)
                + ",\"type\":" + type(it.type()) + ",\"aborts\":" + aborts(it) + "}";
    }

    /**
     * A call, and what it reaches.
     *
     * <p>What a residual call reaches is one of four things and the checker has already decided
     * which: a definition the module holds, a value that runs where it is declared, a behavior, or
     * a kernel the language itself implements. A kernel crosses the same as any other reach — its
     * identity, written out — because whether this backend can lower it is a question for the half
     * that lowers, not for this one: keeping a list here of which kernels the driver already
     * answers would be the second table {@link ProgramWriter}'s own class doc rules out.
     *
     * <p>{@code reaches} is one nested object and not {@code declared}/{@code kernel} written as
     * siblings of {@code arguments} and {@code type}: which of the two a reach carries is exactly
     * what {@code is} already says, so a document naming both, or naming {@code is:"kernel"} with
     * no {@code kernel} at all, is a shape the driver's own reader does not parse as a call rather
     * than one it parses and then has to notice is missing something.
     */
    private String call(Core.Call it, Bindings bindings) {
        StringJoiner arguments = new StringJoiner(",", "[", "]");
        for (Core argument : it.args()) {
            arguments.add(core(argument, bindings));
        }
        String reaches = switch (it.fn()) {
            case Core.Reached.OfDeclaration target -> switch (target.reaches()) {
                case Core.Reaches.AHelper held ->
                        "{\"is\":\"helper\",\"declared\":" + quoted(reached(held.declaration())) + "}";
                case Core.Reaches.ABehavior held -> {
                    behaviorsMet.add(held.behavior());
                    yield "{\"is\":\"behavior\",\"declared\":"
                            + quoted(reached(held.declaration())) + "}";
                }
                // A helper or a behavior is the only two `Reaches` `OfDeclaration#reaches` ever
                // settles to; a value's own reference is `OfValue` or `OfPublishedValue` below,
                // never one this compilation resolved a plain declaration to.
                case Core.Reaches.AValue held -> throw new IllegalStateException(
                        "a declaration's reference resolved to the value " + held.declaration());
                case Core.Reaches.APublishedValue held -> throw new IllegalStateException(
                        "a declaration's reference resolved to the published value "
                                + held.declaration());
            };
            // A value that runs where it is declared, and this module is that module: the method
            // it runs as is a local definition, so its identity crosses the way a value's does and
            // not the way a helper's does — split, so `value_symbol` never has to be split back out
            // of a joined spelling.
            case Core.Reached.OfValue target -> value(it, target.denotes());
            // A value another module declares, reached through the entry that module publishes for
            // it. Split identity for the same reason, and its own wire tag: nothing of this
            // value's body or the types it is built from is this module's to write, so a reader
            // must not read it as the same kind of reach a local value is.
            case Core.Reached.OfPublishedValue target -> publishedValue(it, target.denotes());
            case Core.Reached.OfKernel target -> kernel(it, target);
            // An operation this compiler mints after everything is resolved, which no source can
            // write and which stands for a shape a backend knows how to lower.
            case Core.Emitted target -> throw notYet("the operation " + target, it);
        };
        return "{\"core\":\"call\",\"reaches\":" + reaches
                + ",\"arguments\":" + arguments
                + ",\"type\":" + type(it.type()) + ",\"aborts\":" + aborts(it) + "}";
    }

    /**
     * What an operator reads its operands as, which the checker settled and the operands' types do
     * not say: as they stand, in one type for this operator only, or at their exact values.
     */
    private String reading(Core.BinaryReading reading) {
        return switch (reading) {
            case Core.BinaryReading.AsTheyStand it -> "{\"is\":\"astheystand\"}";
            case Core.BinaryReading.In it -> "{\"is\":\"in\",\"type\":" + type(it.type()) + "}";
            case Core.BinaryReading.ExactNumbers it -> "{\"is\":\"exactnumbers\"}";
        };
    }

    /**
     * A kernel the call reaches, with what this application of it takes each argument as and what
     * else the checker settled about it. The kernel's own signature has type variables, and what
     * they came to here is the checker's answer, not something to substitute again downstream.
     */
    private String kernel(Core.Call call, Core.Reached.OfKernel target) {
        if (!(call.settlement() instanceof Core.CallSettlement.AtKernel settled)) {
            throw new IllegalStateException("a kernel's application carries what it takes: " + call);
        }
        StringJoiner takes = new StringJoiner(",", "[", "]");
        for (Type taken : settled.takes()) {
            takes.add(type(taken));
        }
        String fact = switch (settled.fact()) {
            case Core.KernelFact.None it -> "{\"is\":\"none\"}";
            case Core.KernelFact.StringMatches it ->
                    "{\"is\":\"stringmatches\",\"pattern\":" + quoted(it.pattern()) + "}";
            case Core.KernelFact.OrderingSubject it ->
                    "{\"is\":\"orderingsubject\",\"type\":" + type(it.type()) + "}";
        };
        return "{\"is\":\"kernel\",\"kernel\":" + quoted(target.kernel().key())
                + ",\"takes\":" + takes + ",\"fact\":" + fact + "}";
    }

    /** A call reaching the value {@code denotes}, split into the module that declares it and its
     *  own name — what a native `value_symbol` is built from. */
    private String value(Core.Call it, ValueName denotes) {
        ValueName.Helper value = valueDenoted(it, denotes);
        return "{\"is\":\"value\",\"module\":" + quoted(value.module())
                + ",\"name\":" + quoted(value.name()) + "}";
    }

    /** As {@link #value}, for a call reaching another module's published entry for a value. */
    private String publishedValue(Core.Call it, ValueName denotes) {
        ValueName.Helper value = valueDenoted(it, denotes);
        return "{\"is\":\"publishedvalue\",\"module\":" + quoted(value.module())
                + ",\"name\":" + quoted(value.name()) + "}";
    }

    /** What a value's reference denotes. Never anything else: {@link Core.Reached.OfValue} and
     *  {@link Core.Reached.OfPublishedValue} both refuse to be built over a reference that does
     *  not resolve to one. */
    private static ValueName.Helper valueDenoted(Core.Call it, ValueName denotes) {
        if (denotes instanceof ValueName.Helper value) {
            return value;
        }
        throw new IllegalStateException("a value's reference resolved to " + denotes + ": " + it);
    }

    /**
     * A fork on what a value is, arm by arm.
     *
     * <p>An arm crosses as what it tests and what it reads, both as the checker resolved them.
     * What a case comes to is not worked out again here: a case that is itself a sum stands for the
     * leaves under it, and those leaves are what the arm tests against, so the atoms are written
     * rather than the name they were written under.
     *
     * <p>The binder is numbered before the body is written, because the body reads it. An arm that
     * binds nothing has no number, which is a different thing from binding something nothing reads.
     */
    private String match(Core.Match it, Bindings bindings) {
        StringJoiner arms = new StringJoiner(",", "[", "]");
        for (Core.Case arm : it.cases()) {
            StringJoiner selects = new StringJoiner(",", "[", "]");
            for (ResolvedCase selected : arm.pattern().cases()) {
                selects.add(selects(selected));
            }
            String binding = arm.binder() == null
                    ? "null"
                    : Integer.toString(bindings.number(arm.binder().binding()));
            // What the value is read as inside the arm, which the checker settled and nothing
            // downstream can work out from what the arm tests: an optional's present carrier is
            // tested the same way whatever it holds.
            String binds = arm.binder() == null
                    ? "null"
                    : type(arm.pattern().bindType());
            arms.add("{\"selects\":" + selects + ",\"binding\":" + binding + ",\"binds\":" + binds
                    + ",\"body\":" + core(arm.body(), bindings) + "}");
        }
        return "{\"core\":\"match\",\"subject\":" + core(it.scrutinee(), bindings)
                + ",\"arms\":" + arms + ",\"type\":" + type(it.type())
                + ",\"aborts\":" + aborts(it) + "}";
    }

    /** What one case of an arm tests for, and what it leaves to be read. */
    private String selects(ResolvedCase selected) {
        return switch (selected.refinement()) {
            case Refinement.Direct it -> {
                StringJoiner atoms = new StringJoiner(",", "[", "]");
                for (TypeSymbol atom : selected.atoms()) {
                    atoms.add(caseOf(atom));
                }
                yield "{\"tests\":\"which\",\"atoms\":" + atoms + "}";
            }
            case Refinement.OptionPresent it -> "{\"tests\":\"held\"}";
            case Refinement.OptionAbsent it -> "{\"tests\":\"nothing\"}";
        };
    }

    private static NotLowered notYet(String what, Core node) {
        return new NotLowered(what + " at " + node.pos());
    }

    /**
     * How an operator is spelt on the wire.
     *
     * <p>Every one of them, written out rather than taken from the name the enum happens to carry.
     * A name is a spelling and this is a vocabulary: written as {@code name()}, an operator added
     * to the language would cross to a reader that has never heard of it, and the first thing to
     * notice would be the far side failing to parse a document this side thought it had written.
     */
    private static String op(BinOp op) {
        return switch (op) {
            case EQ -> "EQ";
            case NE -> "NE";
            case LT -> "LT";
            case LE -> "LE";
            case GT -> "GT";
            case GE -> "GE";
            case AND -> "AND";
            case OR -> "OR";
            case ADD -> "ADD";
            case SUB -> "SUB";
            case MUL -> "MUL";
            case DIV -> "DIV";
            case CONCAT -> "CONCAT";
        };
    }

    /** How a reason a run ends without a value is spelt on the wire, for the same reason and in the
     *  same way. */
    private static String abort(AbortKind kind) {
        return switch (kind) {
            case INVARIANT_NOT_HELD -> "INVARIANT_NOT_HELD";
            case ENSURES_NOT_HELD -> "ENSURES_NOT_HELD";
            case UNREACHABLE_REACHED -> "UNREACHABLE_REACHED";
            case DIVISION_BY_ZERO -> "DIVISION_BY_ZERO";
            case REQUIRED_FORM_HAS_NO_PLACE -> "REQUIRED_FORM_HAS_NO_PLACE";
            case INVALID_BOUNDS -> "INVALID_BOUNDS";
        };
    }

    /**
     * Every reason {@code node} can end without a value for, read off the program rather than
     * re-derived from what kind of node it is or what it does — the same programme {@link #op} and
     * {@link #prim} follow, applied to a question upstream now answers instead of one this writer
     * would otherwise have to.
     *
     * <p>Written for every node this walk successfully crosses and not only the ones that plainly
     * can abort, so that an empty array here is this writer's own answer and not silence a reader
     * could mistake for a question nobody asked. In {@link AbortKind}'s own order, so two programs
     * that abort the same way write the same document.
     */
    private String aborts(Core node) {
        // Asked once and held rather than asked once per member below: abortsAt is a lookup this
        // writer would otherwise repeat AbortKind.values().length times for one node, and every
        // node this walk crosses asks it.
        AbortSet at = program.abortsAt(node);
        StringJoiner kinds = new StringJoiner(",", "[", "]");
        for (AbortKind kind : AbortKind.values()) {
            if (at.contains(kind)) {
                kinds.add(quoted(abort(kind)));
            }
        }
        return kinds.toString();
    }

    /** How a primitive is spelt on the wire, for the same reason and in the same way. */
    private static String prim(Type.Prim prim) {
        return switch (prim) {
            case INT -> "INT";
            case STRING -> "STRING";
            case BOOL -> "BOOL";
            case DECIMAL -> "DECIMAL";
            case RATIONAL -> "RATIONAL";
            case DATE -> "DATE";
            case TIME -> "TIME";
            case DATETIME -> "DATETIME";
            case INSTANT -> "INSTANT";
            case RAW -> "RAW";
        };
    }

    private String type(Type type) {
        return switch (type) {
            case Type.Prim it -> "{\"prim\":" + quoted(prim(it)) + "}";

            case Type.Nothing it -> throw notYet("the type " + it);
            case Type.Never it -> throw notYet("the type " + it);
            case Type.Erroneous it -> throw notYet("the type " + it);
            case Type.Var it -> throw notYet("a type variable");
            case Type.MetaVar it -> throw notYet("a type this compiler left open");
            case Type.Ref it -> "{\"declared\":" + quoted(declaredName(it.name())) + "}";
            case Type.OptionOf it -> "{\"option\":" + type(it.element()) + "}";
            case Type.TupleOf it -> {
                StringJoiner members = new StringJoiner(",", "[", "]");
                for (Type member : it.elements()) {
                    members.add(type(member));
                }
                yield "{\"tuple\":" + members + "}";
            }

            // What a union's members are is what tells one apart from another, and each of them
            // says which type it is. The union itself is written nowhere at run time.
            case Type.Union it -> {
                StringJoiner members = new StringJoiner(",", "[", "]");
                for (TypeSymbol member : it.members()) {
                    members.add(caseOf(member));
                }
                yield "{\"union\":" + members + "}";
            }

            // Written whole, as the checker has them. Whether a value of one has a representation
            // here is the lowering's question, and a writer that refused one would be answering
            // it from the side that never lays a value out.
            case Type.ListOf it -> "{\"list\":" + type(it.element()) + "}";
            case Type.SetOf it -> "{\"set\":" + type(it.element()) + "}";
            case Type.MapOf it -> "{\"map\":{\"key\":" + type(it.key())
                    + ",\"value\":" + type(it.value()) + "}}";
            case Type.FnOf it -> {
                StringJoiner takes = new StringJoiner(",", "[", "]");
                for (Type param : it.params()) {
                    takes.add(type(param));
                }
                yield "{\"fn\":{\"takes\":" + takes + ",\"answers\":" + type(it.result()) + "}}";
            }
        };
    }

    private static NotLowered notYet(String what) {
        return new NotLowered(what);
    }

    private static String quoted(String text) {
        StringBuilder out = new StringBuilder(text.length() + 2).append('"');
        for (int i = 0; i < text.length(); i++) {
            char c = text.charAt(i);
            switch (c) {
                case '"' -> out.append("\\\"");
                case '\\' -> out.append("\\\\");
                case '\n' -> out.append("\\n");
                case '\r' -> out.append("\\r");
                case '\t' -> out.append("\\t");
                default -> {
                    if (c < 0x20) {
                        out.append("\\u%04x".formatted((int) c));
                    } else {
                        out.append(c);
                    }
                }
            }
        }
        return out.append('"').toString();
    }
}
