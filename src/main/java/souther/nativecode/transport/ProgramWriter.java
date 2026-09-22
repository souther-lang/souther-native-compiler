package souther.nativecode.transport;

import souther.compiler.core.Core;
import souther.compiler.core.ValueShape;
import souther.compiler.observe.ObservedValue;
import souther.compiler.observe.RowStatement;
import souther.compiler.program.BehaviorTarget;
import souther.compiler.program.CheckedBehavior;
import souther.compiler.program.CheckedData;
import souther.compiler.program.CheckedHelper;
import souther.compiler.program.CheckedImplementation;
import souther.compiler.program.CheckedModule;
import souther.compiler.program.CheckedProgram;
import souther.compiler.program.CheckedRow;
import souther.compiler.program.Declared;
import souther.compiler.program.DeclaredBy;
import souther.compiler.program.Publication;
import souther.compiler.types.BinOp;
import souther.compiler.types.BindingId;
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
 * already been answered upstream — what a call reaches, what an arm binds, what a function
 * captures. A writer that started answering one of them would be a second place the language means
 * something, and the two would come apart.
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
    public static final int TRANSPORT_VERSION = 3;

    private final CheckedProgram program;

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
     * <p>Four vocabularies and not six. What a behavior does instead of carrying a body, and what a
     * call reaches, are switches over shapes rather than over an enum, so there is no member to ask
     * for the spelling of without an instance of one to hand. Those cross under a real program or
     * not at all.
     */
    public static String vocabularies() {
        return "{\"transport\":" + TRANSPORT_VERSION
                + ",\"op\":" + spellings(BinOp.values(), ProgramWriter::op)
                + ",\"prim\":" + spellings(Type.Prim.values(), ProgramWriter::prim)
                + ",\"publication\":" + spellings(Publication.values(), ProgramWriter::publication)
                + ",\"declaredby\":" + spellings(DeclaredBy.values(), ProgramWriter::by)
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
     * <p>What is not written is whether the module publishes the type. That is the same question
     * the object already asks of a behavior, and {@link CheckedModule#publicationOf} answers it for
     * a behavior and for nothing else, so there is nothing here to project. Until there is, an
     * object exports the token of every type it declares, including one the module keeps.
     */
    private String declaration(TypeSymbol.AtModule name, Declared declared) {
        // What a field holds is a type too, and it may be one nothing else in the document has
        // named. Met here rather than left to whoever reads the field, because a declaration is
        // where a type stops being reachable from anything but itself.
        if (declared.data() instanceof CheckedData.WithFields held) {
            for (ValueShape.Field field : held.fields()) {
                type(field.type());
            }
        }
        String identity = "{\"module\":" + quoted(name.module())
                + ",\"name\":" + quoted(name.name())
                + ",\"by\":" + quoted(by(declared.declaredBy()));
        return switch (declared.data()) {
            case CheckedData.Product it -> identity
                    + ",\"is\":\"product\",\"fields\":" + fieldNames(it.fields())
                    + ",\"invariants\":" + it.invariants().size() + "}";
            // A newtype holds one value and is told apart from a product of one field by what may
            // be written of it, which is the checker's business and settled before this.
            case CheckedData.Newtype it -> identity
                    + ",\"is\":\"newtype\",\"fields\":" + fieldNames(it.fields())
                    + ",\"invariants\":" + it.invariants().size() + "}";
            case CheckedData.Unit it -> identity
                    + ",\"is\":\"unit\",\"fields\":[],\"invariants\":0}";
            // A sum is never built, so it has no fields of its own; what it says is which types
            // stand as its cases, and a case may be a sum again.
            case CheckedData.Sum it -> {
                StringJoiner cases = new StringJoiner(",", "[", "]");
                for (TypeSymbol held : it.cases()) {
                    cases.add(quoted(symbol(held)));
                }
                yield identity + ",\"is\":\"sum\",\"cases\":" + cases + "}";
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

    private String fieldNames(List<ValueShape.Field> fields) {
        StringJoiner names = new StringJoiner(",", "[", "]");
        for (ValueShape.Field field : fields) {
            names.add(quoted(field.name()));
        }
        return names.toString();
    }

    private String module(CheckedModule module) {
        StringJoiner bodies = new StringJoiner(",", "[", "]");
        for (CheckedBehavior behavior : module.behaviors()) {
            if (behavior.implementation() instanceof CheckedImplementation.Body written) {
                bodies.add(body(module, behavior, written));
            }
        }
        StringJoiner helpers = new StringJoiner(",", "[", "]");
        for (CheckedHelper helper : module.helpers()) {
            helpers.add(helper(helper));
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
        return "{\"name\":" + quoted(module.name())
                + ",\"helpers\":" + helpers
                + ",\"bodies\":" + bodies
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
        return "{\"core\":\"call\",\"reaches\":\"behavior\""
                + ",\"declared\":" + quoted(module.name() + "." + behavior.name().name())
                + ",\"arguments\":" + arguments
                + ",\"type\":" + type(behavior.signature().answers()) + "}";
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
        return switch (value) {
            case ObservedValue.Integer it when at == Type.Prim.INT ->
                    "{\"core\":\"int\",\"value\":" + it.value() + ",\"type\":" + type(at) + "}";
            case ObservedValue.Bool it when at == Type.Prim.BOOL ->
                    "{\"core\":\"bool\",\"value\":" + it.value() + ",\"type\":" + type(at) + "}";

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
     */
    private String helper(CheckedHelper helper) {
        Bindings bindings = new Bindings();
        StringJoiner parameters = new StringJoiner(",", "[", "]");
        StringJoiner takes = new StringJoiner(",", "[", "]");
        for (CheckedHelper.Parameter parameter : helper.parameters()) {
            bindings.number(parameter.binder().binding());
            parameters.add(quoted(parameter.binder().name()));
            takes.add(type(parameter.type()));
        }
        return "{\"declared\":" + quoted(reached(helper.declares()))
                + ",\"parameters\":" + parameters
                + ",\"takes\":" + takes
                + ",\"answers\":" + type(helper.body().type())
                + ",\"body\":" + core(helper.body(), bindings)
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

    /** The same for a name that may not be a module's, which is refused rather than guessed at. */
    private String symbol(TypeSymbol name) {
        return switch (name) {
            case TypeSymbol.AtModule it -> named(it);
            // A primitive standing as a case of a union, and a case the language gives. Both are
            // cases with no declaration to be made of, and nothing here builds or reads one yet.
            case TypeSymbol.Primitive it -> throw notYet("the primitive case " + it.name());
            case TypeSymbol.LanguageCase it -> throw notYet("the case " + it.name());
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
        StringJoiner takes = new StringJoiner(",", "[", "]");
        for (Type type : behavior.signature().takes()) {
            takes.add(type(type));
        }
        // The module and the name apart, which is what an identity is made of and what a symbol is
        // built from. Joined into one string it would have to be split back, and a module's name
        // carries dots.
        return "{\"module\":" + quoted(name.module())
                + ",\"name\":" + quoted(name.name())
                + ",\"is\":" + quoted(how)
                + ",\"takes\":" + takes
                + ",\"answers\":" + type(behavior.signature().answers())
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
        return "{\"declared\":" + quoted(module.name() + "." + behavior.name().name())
                + ",\"parameters\":" + parameters
                + ",\"publication\":" + quoted(publication(module.publicationOf(behavior.name())))
                + ",\"body\":" + core(written.body(), bindings)
                + "}";
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
                    + ",\"type\":" + type(it.type()) + "}";
            case Core.Read it -> "{\"core\":\"read\",\"binding\":"
                    + bindings.of(it.binding(), it.name())
                    + ",\"type\":" + type(it.type()) + "}";
            case Core.Bool it -> "{\"core\":\"bool\",\"value\":" + it.value()
                    + ",\"type\":" + type(it.type()) + "}";
            case Core.Binary it -> "{\"core\":\"binary\",\"op\":" + quoted(op(it.op()))
                    + ",\"left\":" + core(it.left(), bindings)
                    + ",\"right\":" + core(it.right(), bindings)
                    + ",\"type\":" + type(it.type()) + "}";
            case Core.UnitValue it -> "{\"core\":\"unit\",\"declared\":"
                    + quoted(symbol(it.data())) + ",\"type\":" + type(it.type()) + "}";
            case Core.Construct it -> {
                StringJoiner values = new StringJoiner(",", "[", "]");
                for (Core.FieldValue field : it.values()) {
                    values.add(core(field.value(), bindings));
                }
                yield "{\"core\":\"construct\",\"declared\":" + quoted(named(it.typeName()))
                        + ",\"values\":" + values + ",\"type\":" + type(it.type()) + "}";
            }
            case Core.FieldAccess it -> "{\"core\":\"field\",\"target\":"
                    + core(it.target(), bindings) + ",\"field\":" + quoted(it.field())
                    + ",\"type\":" + type(it.type()) + "}";
            case Core.Match it -> match(it, bindings);
            case Core.OptionSome it -> "{\"core\":\"some\",\"value\":" + core(it.value(), bindings)
                    + ",\"type\":" + type(it.type()) + "}";
            case Core.OptionNone it -> "{\"core\":\"none\",\"type\":" + type(it.type()) + "}";
            case Core.Tuple it -> {
                StringJoiner members = new StringJoiner(",", "[", "]");
                for (Core element : it.elements()) {
                    members.add(core(element, bindings));
                }
                yield "{\"core\":\"tuple\",\"members\":" + members
                        + ",\"type\":" + type(it.type()) + "}";
            }
            case Core.TupleGet it -> "{\"core\":\"member\",\"tuple\":" + core(it.tuple(), bindings)
                    + ",\"at\":" + it.index() + ",\"type\":" + type(it.type()) + "}";
            case Core.Neg it -> "{\"core\":\"neg\",\"operand\":" + core(it.operand(), bindings)
                    + ",\"type\":" + type(it.type()) + "}";
            case Core.LetIn it -> letIn(it, bindings);
            case Core.If it -> "{\"core\":\"if\",\"cond\":" + core(it.cond(), bindings)
                    + ",\"then\":" + core(it.then(), bindings)
                    + ",\"else\":" + core(it.els(), bindings)
                    + ",\"type\":" + type(it.type()) + "}";

            case Core.Decimal it -> throw notYet("a decimal literal", it);
            case Core.Str it -> throw notYet("a string literal", it);
            case Core.Temporal it -> throw notYet("a temporal literal", it);
            case Core.MaterialisedValue it -> throw notYet("a value read from its module", it);
            case Core.Call it -> call(it, bindings);
            case Core.PreservedCall it -> throw notYet("a call kept for what it says", it);
            case Core.Apply it -> throw notYet("an application of a function value", it);
            case Core.IfConstructed it -> throw notYet("an attempted construction", it);
            case Core.Block it -> throw notYet("a function value", it);
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
     */
    private String letIn(Core.LetIn it, Bindings bindings) {
        String value = core(it.value(), bindings);
        int number = bindings.number(it.binder().binding());
        return "{\"core\":\"let\",\"binding\":" + number
                + ",\"value\":" + value
                + ",\"body\":" + core(it.body(), bindings)
                + ",\"type\":" + type(it.type()) + "}";
    }

    /**
     * A call, and what it reaches.
     *
     * <p>What a residual call reaches is one of three things and the checker has already decided
     * which: a definition the module holds, a value that runs where it is declared, or a behavior.
     * A kernel is what the language implements rather than what a program holds, and nothing here
     * runs one yet.
     */
    private String call(Core.Call it, Bindings bindings) {
        StringJoiner arguments = new StringJoiner(",", "[", "]");
        for (Core argument : it.args()) {
            arguments.add(core(argument, bindings));
        }
        String reaches = switch (it.fn()) {
            case Core.Reached.OfDeclaration target -> switch (target.reaches()) {
                case Core.Reaches.AHelper held ->
                        "\"reaches\":\"helper\",\"declared\":" + quoted(reached(held.declaration()));
                case Core.Reaches.APublishedValue held ->
                        "\"reaches\":\"value\",\"declared\":" + quoted(reached(held.declaration()));
                case Core.Reaches.ABehavior held -> {
                    behaviorsMet.add(held.behavior());
                    yield "\"reaches\":\"behavior\",\"declared\":"
                            + quoted(reached(held.declaration()));
                }
            };
            case Core.Reached.OfPublishedValue target ->
                    "\"reaches\":\"value\",\"declared\":" + quoted(reached(target.denotes()));
            case Core.Reached.OfKernel target ->
                    throw notYet("a call to " + target.kernel(), it);
            // An operation this compiler mints after everything is resolved, which no source can
            // write and which stands for a shape a backend knows how to lower.
            case Core.Emitted target -> throw notYet("the operation " + target, it);
        };
        return "{\"core\":\"call\"," + reaches
                + ",\"arguments\":" + arguments
                + ",\"type\":" + type(it.type()) + "}";
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
                + ",\"arms\":" + arms + ",\"type\":" + type(it.type()) + "}";
    }

    /** What one case of an arm tests for, and what it leaves to be read. */
    private String selects(ResolvedCase selected) {
        return switch (selected.refinement()) {
            case Refinement.Direct it -> {
                StringJoiner atoms = new StringJoiner(",", "[", "]");
                for (TypeSymbol atom : selected.atoms()) {
                    atoms.add(quoted(symbol(atom)));
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
            case Type.Ref it -> "{\"declared\":" + quoted(symbol(it.name())) + "}";
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
                    members.add(quoted(symbol(member)));
                }
                yield "{\"union\":" + members + "}";
            }

            case Type.ListOf it -> throw notYet("a list type");
            case Type.MapOf it -> throw notYet("a map type");
            case Type.SetOf it -> throw notYet("a set type");
            case Type.FnOf it -> throw notYet("a function type");
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
