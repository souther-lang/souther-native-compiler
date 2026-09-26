package souther.nativecode.transport;

import souther.compiler.abort.AbortKind;
import souther.compiler.abort.AbortSet;
import souther.compiler.check.CallElaborator;
import souther.compiler.core.Composition;
import souther.compiler.core.Contract;
import souther.compiler.core.Core;
import souther.compiler.core.EnsuresEnforcement;
import souther.compiler.core.Kernel;
import souther.compiler.diag.Region;
import souther.compiler.core.ValueShape;
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
import souther.compiler.program.CheckedSignature;
import souther.compiler.program.CheckedValue;
import souther.compiler.program.CheckedValueEntry;
import souther.compiler.program.Declared;
import souther.compiler.program.DeclaredBy;
import souther.compiler.program.Publication;
import souther.compiler.program.StandsIn;
import souther.compiler.regex.CodePoints;
import souther.compiler.regex.PatternMeaning;
import souther.compiler.types.BinOp;
import souther.compiler.types.BindingId;
import souther.compiler.types.LanguageCaseId;
import souther.compiler.types.LeafScalar;
import souther.compiler.types.MapKeyRepresentation;
import souther.compiler.types.ReachName;
import souther.compiler.types.Refinement;
import souther.compiler.types.ResolvedCase;
import souther.compiler.types.Type;
import souther.compiler.types.TypeSymbol;
import souther.compiler.types.ValueName;
import souther.nativecode.NotLowered;

import java.time.Instant;
import java.time.LocalDate;
import java.time.LocalDateTime;
import java.time.LocalTime;
import java.time.ZoneOffset;
import java.util.ArrayList;
import java.util.Collection;
import java.util.HashMap;
import java.util.LinkedHashMap;
import java.util.LinkedHashSet;
import java.util.List;
import java.util.Map;
import java.util.Optional;
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
    public static final int TRANSPORT_VERSION = 26;

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

    /**
     * The numbers the type variables of the helper being written cross under, and nothing while
     * anything else is written.
     *
     * <p>A helper is the one definition whose body the checker leaves open over variables
     * (ADR-0092), so it is the one place a variable can stand. Anywhere else a variable is a type
     * nobody settled, and it is refused as one. Within a helper a variable is numbered where it is
     * first met, starting again at nought for every helper: two helpers that both spell {@code 'a}
     * do not share a variable, and what binds a variable is a call of the helper that holds it.
     * The number is this document's, the same as a binding's, and says nothing about what the
     * variable comes to.
     */
    private Map<Type.Var, Integer> typeVariables;

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
     * <p>Eight vocabularies and not ten. What a behavior does instead of carrying a body, and what
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
                + ",\"emitted\":" + spellings(Core.Emitted.values(), ProgramWriter::emitted)
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
            // stand as its cases, which the checker has already descended to their leaves.
            case CheckedData.Sum it -> {
                StringJoiner cases = new StringJoiner(",", "[", "]");
                for (TypeSymbol held : it.cases()) {
                    cases.add(identity(held));
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
     *
     * <p>What each of that build's clauses is answered under is written, since an attempted
     * construction here takes the arm the clause names ({@link #headers}).
     */
    private String clauses(Declared declared, CheckedData.WithFields held, Bindings bindings) {
        return switch (declared.declaredBy()) {
            case A_MODULE -> ",\"invariants\":" + invariants(held, bindings);
            case A_MODULE_ON_THE_PATH -> ",\"headers\":" + headers(held);
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

    /**
     * What each clause a value of this has to hold is answered under, in the order a failure is
     * decided in, and nothing of what it says: the name where the author gave one.
     *
     * <p>The name and its place are the declaration's interface. The object of the build that runs
     * the clauses answers which one did not hold by its place, and an arm here names it by the
     * name, so this is what the two are matched through. The condition is that build's own, for
     * the reason {@link #clauses} gives.
     */
    private String headers(CheckedData.WithFields held) {
        StringJoiner written = new StringJoiner(",", "[", "]");
        for (ValueShape.Invariant clause : held.invariants()) {
            String name = clause.name().map(ProgramWriter::quoted).orElse("null");
            written.add("{\"name\":" + name + "}");
        }
        return written.toString();
    }

    /** What a field carries across the boundary, as the check derived it. */
    private String codec(CheckedCodecShape shape) {
        return switch (shape) {
            case CheckedCodecShape.Scalar it ->
                    "{\"is\":\"scalar\",\"scalar\":" + quoted(leaf(it.kind())) + "}";
            case CheckedCodecShape.Named it ->
                    "{\"is\":\"named\",\"named\":" + identity(it.name()) + "}";
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
                    "{\"is\":\"nominal\",\"named\":" + identity(it.name()) + "}";
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
                    "{\"is\":\"nominal\",\"named\":" + identity(it.name()) + "}";
            case CheckedBoundaryOutput.ListOf it ->
                    "{\"is\":\"listof\",\"element\":" + output(it.element()) + "}";
            case CheckedBoundaryOutput.SetOf it ->
                    "{\"is\":\"setof\",\"element\":" + output(it.element()) + "}";
            case CheckedBoundaryOutput.MapOf it -> "{\"is\":\"mapof\",\"key\":" + key(it.key())
                    + ",\"value\":" + output(it.value()) + "}";
            case CheckedBoundaryOutput.Cases it -> {
                StringJoiner cases = new StringJoiner(",", "[", "]");
                for (TypeSymbol held : it.cases()) {
                    cases.add(identity(held));
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
                    "{\"is\":\"namedkey\",\"named\":" + identity(it.name()) + "}";
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
                String entry = example(behavior, at, rows.get(at));
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
     * row is written.
     */
    private String example(CheckedBehavior behavior, int at, CheckedRow row) {
        List<StandsIn> standIns = row.statement() instanceof CheckedRow.WithStandIns it
                ? it.standsIn() : List.of();
        List<CheckedHelper> inputs = switch (row.statement()) {
            case CheckedRow.SelfContained it -> it.inputDefinitions();
            case CheckedRow.WithStandIns it -> it.inputDefinitions();
            // Its answer is owed, which is a row nothing holds an answer to and still a row whose
            // values were read. The entry runs it; what the run answers is nobody's claim yet.
            case CheckedRow.AnswerOwed it -> it.inputDefinitions();
            case CheckedRow.NotReproducible it -> null;
        };
        if (inputs == null) {
            return null;
        }
        StringJoiner standing = new StringJoiner(",", "[", "]");
        for (StandsIn standsIn : standIns) {
            standing.add(standsIn(standsIn));
        }
        return "{\"behavior\":" + quoted(behavior.name().name())
                + ",\"at\":" + at
                + ",\"body\":" + applied(behavior, inputs)
                + ",\"standsIn\":" + standing + "}";
    }

    /**
     * What a row states one dependency answers: each entry, the arguments it states and the answer,
     * in the order the stand-in reads them, and what it answers for the rest, null where it states
     * nothing for the rest.
     *
     * <p>What answers the dependency is the stand-in's rule ({@link StandsIn#answering}): the first
     * entry stating the arguments a call arrived with, compared as the language compares two
     * values, and otherwise the rest. That rule is carried as the entries in order and not as a
     * table keyed by them, since which entry states a call is the comparison's to say and a key
     * would be this writer's.
     *
     * <p>Each value is a call of the definition the checker names for it, as a row's inputs are
     * ({@link #applied}): elaborated at the parameter or the answer of the dependency where it
     * stands, so how it stands there is the checker's and not worked out here from what was
     * observed.
     */
    private String standsIn(StandsIn standsIn) {
        ValueName.Behavior dependency = standsIn.dependency();
        behaviorsMet.add(dependency);
        StringJoiner entries = new StringJoiner(",", "[", "]");
        for (StandsIn.Entry entry : standsIn.entries()) {
            StringJoiner arguments = new StringJoiner(",", "[", "]");
            for (CheckedHelper argument : entry.argumentDefinitions()) {
                arguments.add(computed(argument));
            }
            entries.add("{\"arguments\":" + arguments
                    + ",\"answer\":" + computed(entry.answerDefinition()) + "}");
        }
        String otherwise = switch (standsIn.otherwise()) {
            case StandsIn.Otherwise.Answers it -> computed(it.definition());
            case StandsIn.Otherwise.NothingStated it -> "null";
        };
        return "{\"module\":" + quoted(dependency.module())
                + ",\"name\":" + quoted(dependency.name())
                + ",\"entries\":" + entries
                + ",\"otherwise\":" + otherwise + "}";
    }

    /**
     * A call of a definition the module holds that computes a value a row states, taking nothing.
     *
     * <p>No Core.Call stands behind it for program.abortsAt to ask of, and none is needed: what
     * AbortSites answers for a call reaching a declaration is NONE, whatever it reaches. What the
     * definition ends with is its own, the same answer a call written in a body gets.
     */
    private String computed(CheckedHelper definition) {
        return callNode(helperReach(definition.reachedAs()), List.of(), definition.body().type(),
                AbortSet.NONE);
    }

    /**
     * The one call a row is: the behavior, handed what computes each of the row's inputs.
     *
     * <p>A call and not a shape of its own, so that what an entry does is lowered by whatever
     * lowers a call and the two cannot come apart. Each argument is a call of the definition the
     * row names for that input ({@link CheckedRow.SelfContained#inputDefinitions}), which the module holds as
     * one of its helpers: its body is the operand the row writes, elaborated by the checker at the
     * parameter it is handed to, so how a value stands there — a case where its sum is taken, a
     * value given to an optional field — is the checker's and not worked out here from what the row
     * observed.
     *
     * <p>No Core.Call stands behind these calls for program.abortsAt to ask of, and none is needed:
     * what AbortSites answers for a call reaching a declaration is NONE, whatever it reaches. What
     * the definition or the behavior ends with is its own, the same answer a call written in a body
     * gets.
     */
    private String applied(CheckedBehavior behavior, List<CheckedHelper> inputs) {
        List<String> arguments = new ArrayList<>();
        for (CheckedHelper input : inputs) {
            arguments.add(computed(input));
        }
        return callNode(behaviorReach(behavior.name()), arguments,
                behavior.signature().answers(), AbortSet.NONE);
    }

    /**
     * A definition the module holds as one of its own.
     *
     * <p>A module carries every helper it reaches, including one another module declares, so what
     * is holding it is which module is holding it and what it is called is the reference a call in
     * that module reaches it by ({@link CheckedHelper#reachedAs}). Not where it is declared: the
     * standard library declares {@code foldFrom} and a module reaches it as {@code List.foldFrom},
     * and a call carries the second. Two modules holding one helper hold a copy each, which is what
     * the language says a published helper is.
     *
     * <p>Each parameter is written once, its name and its type together, because they are one
     * {@link CheckedHelper.Parameter}; written as two lists they would be two statements of how many
     * there are. What the helper answers is not written at all: it is its body's type, which the body
     * already carries, and a second copy would only be something a reader has to hold to the first.
     *
     * <p>The reference crosses as what it is, a route and what the route reaches
     * ({@link #reference}), and not as its spelling. The declaration it is a copy of is the second
     * half of it, which what the module is held to is stated over; and a spelling written beside a
     * declaration would be the same fact twice, which {@link souther.compiler.types.ReachName}
     * says of itself.
     *
     * <p>A type in it may be a variable the body leaves open ({@link #typeVariables}). What each one
     * comes to is a call's to say, and every call already carries the types its arguments and its
     * answer were settled at.
     */
    private String helper(CheckedHelper helper) {
        typeVariables = new HashMap<>();
        try {
            Bindings bindings = new Bindings();
            StringJoiner parameters = new StringJoiner(",", "[", "]");
            for (CheckedHelper.Parameter parameter : helper.parameters()) {
                bindings.number(parameter.binder().binding());
                parameters.add("{\"name\":" + quoted(parameter.binder().name())
                        + ",\"type\":" + type(parameter.type()) + "}");
            }
            return "{\"reached\":" + reference(helper.reachedAs())
                    + ",\"parameters\":" + parameters
                    + ",\"body\":" + core(helper.body(), bindings)
                    + "}";
        } finally {
            typeVariables = null;
        }
    }

    /**
     * A reference to a helper, as the checker settled it: the route a module reaches it by, and
     * the declaration the route reaches. A declaration of the module doing the reading is reached
     * as its own, one of another module under that module's name, and an operation of the standard
     * library under the alias the library publishes it as.
     *
     * <p>Written as that structure and not as its spelling, so a reader holding a call and a reader
     * holding the helper compare one value, and what a module is held to about its helpers is
     * asked of the declaration inside it. The declaration is the module's and its name apart, the
     * way a value's identity crosses.
     *
     * <p>A helper is reached over a declaration a module declares as a helper, or over an operation
     * of the library, and over nothing else ({@link Core.Reached.OfDeclaration#reaches}); anything
     * else here is this writer holding something that is not a helper.
     */
    private static String reference(ReachName.Declaration reference) {
        return switch (reference) {
            case ReachName.Own it -> "{\"is\":\"own\"," + helperDeclaration(it.denotes()) + "}";
            case ReachName.OfModule it ->
                    "{\"is\":\"ofmodule\"," + helperDeclaration(it.denotes()) + "}";
            case ReachName.OfLibrary it -> "{\"is\":\"library\",\"alias\":"
                    + quoted(it.denotes().alias()) + ",\"name\":" + quoted(it.denotes().name()) + "}";
        };
    }

    private static String helperDeclaration(ValueName.OfAModule declared) {
        return switch (declared) {
            case ValueName.Helper it ->
                    "\"module\":" + quoted(it.module()) + ",\"name\":" + quoted(it.name());
            case ValueName.Behavior it ->
                    throw new IllegalStateException("a helper reached over the behavior " + it);
        };
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

    /**
     * How a behavior is named on the wire: its module, then its own name.
     *
     * <p>A behavior and nothing wider. What a call reaches the checker has already answered as one
     * of {@link Core.Reaches}' arms, and the one of them that names a behavior names it by
     * {@link ValueName.Behavior}. A name in scope, or one the standard library declares, is not a
     * behavior some module holds, and taking one here would be taking a name this writer would
     * then have to resolve.
     */
    private static String reached(ValueName.Behavior name) {
        return name.module() + "." + name.name();
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
     * Which type a {@link TypeSymbol} is: one a module declares, a primitive standing as one, or
     * one the language gives. Written wherever the checker hands a {@code TypeSymbol} — a union's
     * member, a sum's case, a type reference, what a field or a key or a boundary is named by, a
     * unit — so the three cross as the one sum they are upstream, and an arm added there stops
     * this compiling and the far side reading. Where the checker has already narrowed the name to
     * a declaration, {@link #named} writes the declaration's key instead.
     *
     * <p>The identity and not the shape. How a case is written at a boundary is read on the far
     * side off what the identity reaches — a declaration's arm, or a primitive's being one — so
     * nothing about the shape crosses here that the identity does not already answer. Whether a
     * value of a primitive or a language case named here has a representation is the lowering's
     * to say, not this writer's.
     */
    private String identity(TypeSymbol name) {
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
     *
     * <p>With what constructing it requires, which is part of reaching it and not of how it is
     * written: a caller hands a behavior the capabilities of what it requires, and a composition
     * hands a stage those of the stage's, whichever build implements the stage. Empty for one a host
     * implements, which Souther does not construct, and read together with how it answers.
     */
    private String target(ValueName.Behavior name, BehaviorTarget behavior) {
        String how = switch (behavior.implementation()) {
            case CheckedImplementation.Body it -> "body";
            case CheckedImplementation.Injected it -> "injected";
            case CheckedImplementation.ImplementedElsewhere it -> "elsewhere";
            case CheckedImplementation.Composed it -> "composed";
            case CheckedImplementation.Unwritten it -> "unwritten";
        };
        // The module and the name apart, which is what an identity is made of and what a symbol is
        // built from. Joined into one string it would have to be split back, and a module's name
        // carries dots.
        return "{\"module\":" + quoted(name.module())
                + ",\"name\":" + quoted(name.name())
                + ",\"is\":" + quoted(how)
                + ",\"parameters\":" + parameters(behavior.signature())
                + ",\"output\":" + output(behavior.signature().output())
                + ",\"ensures\":" + ensures(enforcement(name))
                + ",\"requirements\":" + requirements(behavior.requirements())
                + "}";
    }

    /**
     * What a behavior takes, with the names its declaration gives them where it gives any.
     *
     * <p>The names are the signature's and not the binders of a {@code let} implementing it, which
     * correspond by place and may be named otherwise. A composition declares no parameters, and
     * takes what it takes with no name for any of it; which of the two a behavior is crosses as
     * the form the list is written in, so a name and the input it names are one member and there
     * are never two lists to line up.
     */
    private String parameters(CheckedSignature signature) {
        StringJoiner parameters = new StringJoiner(",", "[", "]");
        Optional<List<CheckedSignature.Parameter>> declared = signature.declaredParameters();
        if (declared.isPresent()) {
            for (CheckedSignature.Parameter parameter : declared.get()) {
                parameters.add("{\"name\":" + quoted(parameter.name())
                        + ",\"input\":" + input(parameter.input()) + "}");
            }
            return "{\"named\":" + parameters + "}";
        }
        for (CheckedBoundaryInput input : signature.inputs()) {
            parameters.add(input(input));
        }
        return "{\"positional\":" + parameters + "}";
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

    /**
     * What constructing a behavior requires injected, in the order the checker answered it.
     *
     * <p>The checker's list as it is and not worked out here from what the body calls: a
     * composition requires what its stages do, which is not what it calls, and a second reading
     * of that would come apart from the first. Each is the module and the name apart, as a value
     * is referred to, since a module's name carries dots. Met like a call, so the table of targets
     * says what each one is.
     */
    private String requirements(List<ValueName.Behavior> requirements) {
        StringJoiner required = new StringJoiner(",", "[", "]");
        for (ValueName.Behavior dependency : requirements) {
            behaviorsMet.add(dependency);
            required.add("{\"module\":" + quoted(dependency.module())
                    + ",\"name\":" + quoted(dependency.name()) + "}");
        }
        return required.toString();
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
                    accepted.add(identity(type));
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

    // How each node is spelt, written once whether the checker wrote it or a row's entry makes it,
    // so no node comes to be spelt two ways.

    private String intNode(long value, Type type, AbortSet aborts) {
        return "{\"core\":\"int\",\"value\":" + value
                + ",\"type\":" + type(type) + ",\"aborts\":" + spelled(aborts) + "}";
    }

    private String boolNode(boolean value, Type type, AbortSet aborts) {
        return "{\"core\":\"bool\",\"value\":" + value
                + ",\"type\":" + type(type) + ",\"aborts\":" + spelled(aborts) + "}";
    }

    private String stringNode(String value, Type type, AbortSet aborts) {
        return "{\"core\":\"string\",\"value\":" + quoted(value)
                + ",\"type\":" + type(type) + ",\"aborts\":" + spelled(aborts) + "}";
    }

    /**
     * A {@code Decimal} literal as the checker read it: the integer and the scale, and not the text
     * it was written as. The integer has as many digits as it has, so it crosses as the digits of
     * one, and the scale is the 32-bit number a {@code Decimal}'s scale is.
     */
    private String decimalNode(java.math.BigDecimal value, Type type, AbortSet aborts) {
        return "{\"core\":\"decimal\",\"unscaled\":" + quoted(value.unscaledValue().toString())
                + ",\"scale\":" + value.scale()
                + ",\"type\":" + type(type) + ",\"aborts\":" + spelled(aborts) + "}";
    }

    /**
     * A {@code Date}, {@code Time}, {@code DateTime} or {@code Instant} literal as the checker read
     * it: the count the value it parsed to is, and not the text it was written as.
     *
     * <p>Read by the checker's own parse ({@code CallElaborator#parseTemporal}, which is public for
     * a backend to share the one reading of the text): {@code java.time}'s, whose spellings are more
     * than the ones it writes back ({@code DateTime("2026-07-01t09:30")} is admitted), and which
     * decides what a program may say. The text handed over as it stands would be read a second time
     * on the other side by a grammar of its own, and whatever that one refused of what this one
     * admitted would be a program the checker passed and the backend did not; so what crosses is
     * what was read, as a {@code Decimal}'s integer and scale are. A {@code Date} crosses as its
     * day, a {@code Time} as its second of the day, a {@code DateTime} as its second counted from
     * 1970-01-01T00:00:00 as though it were in UTC, and an {@code Instant} as its second and its
     * nanosecond; the last two are the checker's own carriers ({@code numeric.DateTimes}, {@code
     * numeric.Instants}), and no zone is a claim of either.
     */
    private String temporalNode(Core.Temporal it, AbortSet aborts) {
        Object read = CallElaborator.parseTemporal(it.kind(), it.kind().toString(), it.text(),
                Region.point(it.pos()));
        long count;
        int nano = 0;
        switch (read) {
            case LocalDate day -> count = day.toEpochDay();
            case LocalTime time -> count = time.toSecondOfDay();
            case LocalDateTime dateTime -> count = dateTime.toEpochSecond(ZoneOffset.UTC);
            case Instant moment -> {
                count = moment.getEpochSecond();
                nano = moment.getNano();
            }
            default -> throw new IllegalStateException(
                    "a temporal literal reads as a temporal: " + read);
        }
        return "{\"core\":\"temporal\",\"count\":" + count + ",\"nano\":" + nano
                + ",\"type\":" + type(it.type()) + ",\"aborts\":" + spelled(aborts) + "}";
    }

    private String unitNode(String identity, Type type, AbortSet aborts) {
        return "{\"core\":\"unit\",\"unit\":" + identity
                + ",\"type\":" + type(type) + ",\"aborts\":" + spelled(aborts) + "}";
    }

    private String constructNode(String declared, List<String> values, Type type,
                                 AbortSet aborts) {
        return "{\"core\":\"construct\",\"declared\":" + quoted(declared)
                + ",\"values\":" + joined(values)
                + ",\"type\":" + type(type) + ",\"aborts\":" + spelled(aborts) + "}";
    }

    private String someNode(String value, Type type, AbortSet aborts) {
        return "{\"core\":\"some\",\"value\":" + value
                + ",\"type\":" + type(type) + ",\"aborts\":" + spelled(aborts) + "}";
    }

    private String noneNode(Type type, AbortSet aborts) {
        return "{\"core\":\"none\",\"type\":" + type(type)
                + ",\"aborts\":" + spelled(aborts) + "}";
    }

    private String listNode(List<String> elements, Type type, AbortSet aborts) {
        return "{\"core\":\"list\",\"elements\":" + joined(elements)
                + ",\"type\":" + type(type) + ",\"aborts\":" + spelled(aborts) + "}";
    }

    private String widenNode(String value, Type type, AbortSet aborts) {
        return "{\"core\":\"widen\",\"value\":" + value
                + ",\"type\":" + type(type) + ",\"aborts\":" + spelled(aborts) + "}";
    }

    private String callNode(String reaches, List<String> arguments, Type type, AbortSet aborts) {
        return "{\"core\":\"call\",\"reaches\":" + reaches
                + ",\"arguments\":" + joined(arguments)
                + ",\"type\":" + type(type) + ",\"aborts\":" + spelled(aborts) + "}";
    }

    private String core(Core node, Bindings bindings) {
        return switch (node) {
            case Core.Int it -> intNode(it.value(), it.type(), program.abortsAt(it));
            case Core.Read it -> "{\"core\":\"read\",\"binding\":"
                    + bindings.of(it.binding(), it.name())
                    + ",\"type\":" + type(it.type()) + ",\"aborts\":" + aborts(it) + "}";
            case Core.Bool it -> boolNode(it.value(), it.type(), program.abortsAt(it));
            // The text as the compiler read it, which is the text normalized to NFC. Nothing here
            // folds it a second time: where text arrives from outside is where that is done, and a
            // source file is one of the two places it arrives.
            case Core.Str it -> stringNode(it.value(), it.type(), program.abortsAt(it));
            case Core.Binary it -> "{\"core\":\"binary\",\"op\":" + quoted(op(it.op()))
                    + ",\"reading\":" + reading(it.reading())
                    + ",\"left\":" + core(it.left(), bindings)
                    + ",\"right\":" + core(it.right(), bindings)
                    + ",\"type\":" + type(it.type()) + ",\"aborts\":" + aborts(it) + "}";
            case Core.UnitValue it ->
                    unitNode(identity(it.data()), it.type(), program.abortsAt(it));
            case Core.Construct it -> {
                List<String> values = new ArrayList<>();
                for (Core.FieldValue field : it.values()) {
                    values.add(core(field.value(), bindings));
                }
                yield constructNode(named(it.typeName()), values, it.type(),
                        program.abortsAt(it));
            }
            case Core.FieldAccess it -> "{\"core\":\"field\",\"target\":"
                    + core(it.target(), bindings) + ",\"field\":" + quoted(it.field())
                    + ",\"type\":" + type(it.type()) + ",\"aborts\":" + aborts(it) + "}";
            case Core.Match it -> match(it, bindings);
            case Core.OptionSome it ->
                    someNode(core(it.value(), bindings), it.type(), program.abortsAt(it));
            case Core.OptionNone it -> noneNode(it.type(), program.abortsAt(it));
            case Core.Tuple it -> {
                StringJoiner members = new StringJoiner(",", "[", "]");
                for (Core element : it.elements()) {
                    members.add(core(element, bindings));
                }
                yield "{\"core\":\"tuple\",\"members\":" + members
                        + ",\"type\":" + type(it.type()) + ",\"aborts\":" + aborts(it) + "}";
            }
            case Core.ListLit it -> {
                List<String> elements = new ArrayList<>();
                for (Core element : it.elements()) {
                    elements.add(core(element, bindings));
                }
                yield listNode(elements, it.type(), program.abortsAt(it));
            }
            case Core.TupleGet it -> "{\"core\":\"member\",\"tuple\":" + core(it.tuple(), bindings)
                    + ",\"at\":" + it.index() + ",\"type\":" + type(it.type())
                    + ",\"aborts\":" + aborts(it) + "}";
            case Core.Neg it -> "{\"core\":\"neg\",\"operand\":" + core(it.operand(), bindings)
                    + ",\"type\":" + type(it.type()) + ",\"aborts\":" + aborts(it) + "}";
            case Core.LetIn it -> letIn(it, bindings);
            // Where the checker let a value stand as a type other than its own, which it decided
            // and this writes: what is evaluated, and the type the position takes it as.
            case Core.Widen it ->
                    widenNode(core(it.value(), bindings), it.type(), program.abortsAt(it));
            case Core.If it -> "{\"core\":\"if\",\"cond\":" + core(it.cond(), bindings)
                    + ",\"then\":" + core(it.then(), bindings)
                    + ",\"else\":" + core(it.els(), bindings)
                    + ",\"type\":" + type(it.type()) + ",\"aborts\":" + aborts(it) + "}";

            case Core.Decimal it -> decimalNode(it.value(), it.type(), program.abortsAt(it));
            case Core.Temporal it -> temporalNode(it, program.abortsAt(it));
            // What the checker builds for an analysis to read, and not for a backend to run: a
            // value's build standing as its template, and a call kept standing for what it says.
            // The tree a checked program hands a backend keeps neither — the checker's own emitter
            // refuses both as a tree it was not meant to be handed — so one here is that premise
            // not holding, and not a node this writer is behind on.
            case Core.MaterialisedValue it -> throw new IllegalStateException(
                    "the tree a checked program runs holds no build of a value, and this holds one"
                            + " of " + it.value() + " at " + it.pos());
            case Core.Call it -> call(it, bindings);
            case Core.PreservedCall it -> throw it.unexpectedIn("a backend's writer");
            case Core.Apply it -> apply(it, bindings);
            case Core.IfConstructed it -> attempt(it, bindings);
            case Core.Block it -> block(it, bindings);
            // Where the run ends, and why, in the author's words. The reason crosses though the
            // status a run ends with has no room for it: what a run can say when it ends is the
            // runtime's to widen, and a document that dropped the reason would have to move then.
            case Core.Unreachable it -> "{\"core\":\"unreachable\",\"reason\":"
                    + quoted(it.reason())
                    + ",\"type\":" + type(it.type()) + ",\"aborts\":" + aborts(it) + "}";
        };
    }

    /**
     * An attempted construction: the fields, what the value built is bound under where every clause
     * holds and the branch that reads it, and each departure with the clause it answers.
     *
     * <p>Not a construction under a fork. What {@link Core.IfConstructed#construct} is on its own is
     * a construction that ends the run where a clause does not hold, and here it never does, which
     * is what the checker says of it too ({@code AbortSites} files it as ending nothing). Written as
     * one node, the far side reads one thing that decides which way the run goes, and meets no
     * construction whose meaning is changed by what stands over it.
     *
     * <p>The fields are written before the binder is numbered and the branch after, as a
     * {@link #letIn} writes its value and its body: the value is built out of what was in scope,
     * and exists only in the branch. What the binder is in force at is written beside it, as a
     * {@code let}'s is. A departure is written after the branch and does not read the binder, since
     * where it is taken nothing was built.
     */
    private String attempt(Core.IfConstructed it, Bindings bindings) {
        Core.Construct construct = it.construct();
        StringJoiner values = new StringJoiner(",", "[", "]");
        for (Core.FieldValue field : construct.values()) {
            values.add(core(field.value(), bindings));
        }
        int binding = bindings.number(it.binder().binding());
        String then = core(it.then(), bindings);
        StringJoiner departures = new StringJoiner(",", "[", "]");
        for (Core.ElseArm departure : it.els()) {
            String clause = departure.clause().map(ProgramWriter::quoted).orElse("null");
            departures.add("{\"clause\":" + clause
                    + ",\"body\":" + core(departure.body(), bindings) + "}");
        }
        return "{\"core\":\"attempt\",\"declared\":" + quoted(named(construct.typeName()))
                + ",\"values\":" + values
                + ",\"binding\":" + binding
                + ",\"binds\":" + type(construct.type())
                + ",\"then\":" + then
                + ",\"departures\":" + departures
                + ",\"type\":" + type(it.type()) + ",\"aborts\":" + aborts(it) + "}";
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
        List<String> arguments = new ArrayList<>();
        for (Core argument : it.args()) {
            arguments.add(core(argument, bindings));
        }
        String reaches = switch (it.fn()) {
            case Core.Reached.OfDeclaration target -> switch (target.reaches()) {
                // Under the reference the call reaches it by, which is what the helper the module
                // holds is written under too ({@link #helper}): the two are one reference, and a
                // name made up out of the declaration would not be the one the module holds.
                case Core.Reaches.AHelper ignored -> helperReach(target.name());
                case Core.Reaches.ABehavior held -> behaviorReach(held.behavior());
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
            // write and which stands for a shape a backend lowers whole. It crosses as the member
            // it is and not as what it renders as, which is for a report to quote; whether this
            // backend lowers it is the driver's to say, as it is for a kernel.
            case Core.Emitted target -> "{\"is\":\"emitted\",\"operation\":"
                    + quoted(emitted(target)) + "}";
        };
        return callNode(reaches, arguments, it.type(), program.abortsAt(it));
    }

    /** What a call reaching a helper names it by, for a call a body writes and one a row makes. */
    private static String helperReach(ReachName.Declaration reached) {
        return "{\"is\":\"helper\",\"reached\":" + reference(reached) + "}";
    }

    /** What a call reaching {@code behavior} names it by, for a call a body writes and one a row is. */
    private String behaviorReach(ValueName.Behavior behavior) {
        behaviorsMet.add(behavior);
        return "{\"is\":\"behavior\",\"declared\":" + quoted(reached(behavior)) + "}";
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
                    "{\"is\":\"stringmatches\",\"written\":" + quoted(it.written())
                            + ",\"meaning\":" + meaning(it.meaning()) + "}";
            case Core.KernelFact.OrderingSubject it ->
                    "{\"is\":\"orderingsubject\",\"type\":" + type(it.type()) + "}";
        };
        return "{\"is\":\"kernel\",\"kernel\":" + quoted(target.kernel().key())
                + ",\"takes\":" + takes + ",\"fact\":" + fact + "}";
    }

    /**
     * Which strings a pattern accepts, as the checker read them: its parts, each written once
     * after the parts it is made of, the whole last.
     *
     * <p>A list and not a nested object. A pattern may nest its groups as deep as the checker reads
     * one, and a document nesting as deep would be refused by a reader for its depth rather than
     * for anything the pattern says. A part names the parts it is made of by where they stand in
     * the list, which is always before it.
     */
    private static String meaning(PatternMeaning meaning) {
        List<String> parts = new ArrayList<>();
        part(meaning, parts);
        return "[" + String.join(",", parts) + "]";
    }

    /** Writes {@code meaning}'s parts and then {@code meaning}, answering where it stands. */
    private static int part(PatternMeaning meaning, List<String> parts) {
        String written = switch (meaning) {
            case PatternMeaning.Nothing it -> "{\"is\":\"nothing\"}";
            case PatternMeaning.Never it -> "{\"is\":\"never\"}";
            case PatternMeaning.Symbols it -> {
                StringJoiner ranges = new StringJoiner(",", "[", "]");
                for (CodePoints.Range range : it.held().ranges()) {
                    ranges.add("[" + range.from() + "," + range.to() + "]");
                }
                yield "{\"is\":\"symbols\",\"ranges\":" + ranges + "}";
            }
            case PatternMeaning.InTurn it ->
                    "{\"is\":\"inturn\",\"parts\":" + parts(it.parts(), parts) + "}";
            case PatternMeaning.EitherOf it ->
                    "{\"is\":\"eitherof\",\"arms\":" + parts(it.arms(), parts) + "}";
            case PatternMeaning.Repeated it -> {
                int what = part(it.what(), parts);
                yield "{\"is\":\"repeated\",\"what\":" + what + ",\"least\":" + it.least()
                        + ",\"most\":" + (it.unbounded() ? "null" : Integer.toString(it.most()))
                        + "}";
            }
        };
        parts.add(written);
        return parts.size() - 1;
    }

    private static String parts(List<PatternMeaning> each, List<String> parts) {
        StringJoiner at = new StringJoiner(",", "[", "]");
        for (PatternMeaning one : each) {
            at.add(Integer.toString(part(one, parts)));
        }
        return at.toString();
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
            // A binder over a test that leaves nothing to read, `None as n`, is admitted with no
            // type for what it binds (souther-lang/souther#1984). Nothing it could be written as is
            // one the checker settled, so it is refused until the checker says.
            if (arm.binder() != null && arm.pattern().bindType() == null) {
                throw notYet("an arm binding a name to what it tests holds nothing, which the "
                        + "checker gives no type (souther-lang/souther#1984)", it);
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
                    atoms.add(identity(atom));
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
        return spelled(program.abortsAt(node));
    }

    /** What {@link #aborts} writes, for an answer already asked of the program. */
    private static String spelled(AbortSet at) {
        StringJoiner kinds = new StringJoiner(",", "[", "]");
        for (AbortKind kind : AbortKind.values()) {
            if (at.contains(kind)) {
                kinds.add(quoted(abort(kind)));
            }
        }
        return kinds.toString();
    }

    /** How an operation this compiler emits is spelt on the wire, for the same reason and in the
     *  same way. */
    private static String emitted(Core.Emitted operation) {
        return switch (operation) {
            case BUILD_LIST -> "BUILD_LIST";
            case GROW_LIST -> "GROW_LIST";
            case BUILD_MAP -> "BUILD_MAP";
            case PUT_MAP -> "PUT_MAP";
        };
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

            // The type of what has no value: the element of an empty list literal, and so the
            // accumulator a walk seeded with `[]` starts from. It crosses as a type like any other;
            // that nothing of it is ever laid out is the driver's to act on.
            case Type.Nothing it -> "{\"nothing\":{}}";
            // The type of what does not answer: a computation that ends the run, so nothing that
            // reads a value from here is ever reached. Settled, unlike `Nothing`, which is a type
            // inference has yet to fill in; that no value of it is ever laid out is the driver's.
            case Type.Never it -> "{\"never\":{}}";
            // What the checker stands in where it reported an error and went on. A checked program
            // is one it reported none in, so one here is that premise not holding.
            case Type.Erroneous it -> throw new IllegalStateException(
                    "a checked program holds no type the checker gave up on");
            // A variable crosses only inside the helper that leaves it open, under the number that
            // helper gives it. Anywhere else it is a type the checker did not settle.
            case Type.Var it -> {
                if (typeVariables == null) {
                    throw notYet("a type variable");
                }
                Integer number = typeVariables.get(it);
                if (number == null) {
                    number = typeVariables.size();
                    typeVariables.put(it, number);
                }
                yield "{\"var\":" + number + "}";
            }
            case Type.MetaVar it -> throw notYet("a type this compiler left open");
            case Type.Ref it -> "{\"ref\":" + identity(it.name()) + "}";
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
                    members.add(identity(member));
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
