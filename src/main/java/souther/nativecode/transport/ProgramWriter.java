package souther.nativecode.transport;

import souther.compiler.core.Core;
import souther.compiler.core.ValueShape;
import souther.compiler.program.CheckedBehavior;
import souther.compiler.program.CheckedData;
import souther.compiler.program.CheckedHelper;
import souther.compiler.program.CheckedImplementation;
import souther.compiler.program.CheckedModule;
import souther.compiler.program.CheckedProgram;
import souther.compiler.types.BinOp;
import souther.compiler.types.BindingId;
import souther.compiler.types.Refinement;
import souther.compiler.types.ResolvedCase;
import souther.compiler.types.Type;
import souther.compiler.types.TypeSymbol;
import souther.compiler.types.ValueName;
import souther.nativecode.NotLowered;

import java.util.HashMap;
import java.util.List;
import java.util.Map;
import java.util.StringJoiner;

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
    public static final int TRANSPORT_VERSION = 1;

    private ProgramWriter() {
    }

    /** The whole program as one document. */
    public static String written(CheckedProgram program) {
        StringJoiner declarations = new StringJoiner(",", "[", "]");
        StringJoiner modules = new StringJoiner(",", "[", "]");
        for (CheckedModule module : program.modules()) {
            for (CheckedData declared : module.data()) {
                declarations.add(declaration(declared));
            }
            modules.add(module(module));
        }
        return "{\"transport\":" + TRANSPORT_VERSION
                + ",\"declarations\":" + declarations
                + ",\"modules\":" + modules + "}";
    }

    /**
     * What a declared type is made of.
     *
     * <p>The shape and not the layout: how many fields there are and what they are called, which is
     * the declaration's own answer, while what a field costs and where it sits is the lowering's.
     * A writer that started saying where a field goes would be deciding the representation from the
     * side that never emits one.
     */
    private static String declaration(CheckedData declared) {
        return switch (declared) {
            case CheckedData.Product it -> "{\"declared\":" + quoted(named(it.name()))
                    + ",\"is\":\"product\",\"fields\":" + fieldNames(it.fields())
                    + ",\"invariants\":" + it.invariants().size() + "}";
            // A newtype holds one value and is told apart from a product of one field by what may
            // be written of it, which is the checker's business and settled before this.
            case CheckedData.Newtype it -> "{\"declared\":" + quoted(named(it.name()))
                    + ",\"is\":\"newtype\",\"fields\":" + fieldNames(it.fields())
                    + ",\"invariants\":" + it.invariants().size() + "}";
            case CheckedData.Unit it -> "{\"declared\":" + quoted(named(it.name()))
                    + ",\"is\":\"unit\",\"fields\":[],\"invariants\":0}";
            // A sum is never built, so it has no fields of its own; what it says is which types
            // stand as its cases, and a case may be a sum again.
            case CheckedData.Sum it -> {
                StringJoiner cases = new StringJoiner(",", "[", "]");
                for (TypeSymbol held : it.cases()) {
                    cases.add(quoted(symbol(held)));
                }
                yield "{\"declared\":" + quoted(named(it.name()))
                        + ",\"is\":\"sum\",\"cases\":" + cases + "}";
            }
        };
    }

    private static String fieldNames(List<ValueShape.Field> fields) {
        StringJoiner names = new StringJoiner(",", "[", "]");
        for (ValueShape.Field field : fields) {
            names.add(quoted(field.name()));
        }
        return names.toString();
    }

    private static String module(CheckedModule module) {
        StringJoiner behaviors = new StringJoiner(",", "[", "]");
        for (CheckedBehavior behavior : module.behaviors()) {
            behaviors.add(behavior(behavior));
        }
        StringJoiner helpers = new StringJoiner(",", "[", "]");
        for (CheckedHelper helper : module.helpers()) {
            helpers.add(helper(helper));
        }
        return "{\"name\":" + quoted(module.name())
                + ",\"helpers\":" + helpers
                + ",\"behaviors\":" + behaviors + "}";
    }

    /**
     * A definition the module holds as one of its own.
     *
     * <p>A module carries every helper it reaches, including one another module declares, so what
     * a helper is called here is where it is declared and what it is holding is which module is
     * holding it. Two modules holding one helper hold a copy each, which is what the language says
     * a published helper is.
     */
    private static String helper(CheckedHelper helper) {
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
    private static String reached(ValueName name) {
        return switch (name) {
            case ValueName.OfAModule it -> it.module() + "." + it.name();
            // A name that is in scope where it stands, and one the standard library declares.
            // Neither is a definition a module holds, and nothing here reaches one.
            case ValueName.InScope it -> throw notYet("a call reaching " + it.name());
            case ValueName.Stdlib it -> throw notYet("a call reaching " + it.name());
        };
    }

    /**
     * How a declared type is named on the wire.
     *
     * <p>Its module and then its own name. A module's name carries dots and a type's carries none,
     * so the last segment is the type and no two declarations are spelt the same way.
     */
    private static String named(TypeSymbol.AtModule name) {
        return name.module() + "." + name.name();
    }

    /** The same for a name that may not be a module's, which is refused rather than guessed at. */
    private static String symbol(TypeSymbol name) {
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
    private static String behavior(CheckedBehavior behavior) {
        return switch (behavior.implementation()) {
            case CheckedImplementation.Body it -> withABody(behavior, it);
            case CheckedImplementation.Injected it -> reaching(behavior, "injected");
            case CheckedImplementation.ImplementedElsewhere it -> reaching(behavior, "elsewhere");
            case CheckedImplementation.Composed it -> reaching(behavior, "composed");
            case CheckedImplementation.Unwritten it -> reaching(behavior, "unwritten");
        };
    }

    private static String reaching(CheckedBehavior behavior, String how) {
        StringJoiner takes = new StringJoiner(",", "[", "]");
        for (Type type : behavior.signature().takes()) {
            takes.add(type(type));
        }
        return "{\"name\":" + quoted(behavior.name().name())
                + ",\"is\":" + quoted(how)
                + ",\"takes\":" + takes
                + ",\"answers\":" + type(behavior.signature().answers())
                + "}";
    }

    private static String withABody(CheckedBehavior behavior, CheckedImplementation.Body written) {
        Bindings bindings = new Bindings();
        StringJoiner parameters = new StringJoiner(",", "[", "]");
        for (Core.Binder parameter : written.parameters()) {
            bindings.number(parameter.binding());
            parameters.add(quoted(parameter.name()));
        }
        StringJoiner takes = new StringJoiner(",", "[", "]");
        for (Type type : behavior.signature().takes()) {
            takes.add(type(type));
        }
        return "{\"name\":" + quoted(behavior.name().name())
                + ",\"is\":\"body\""
                + ",\"parameters\":" + parameters
                + ",\"takes\":" + takes
                + ",\"answers\":" + type(behavior.signature().answers())
                + ",\"body\":" + core(written.body(), bindings)
                + "}";
    }

    /**
     * The numbers this document knows a behavior's bindings by.
     *
     * <p>Counted where a binder is written, so a read of one that was never written is a read of
     * something outside what crossed, and it is refused rather than numbered here.
     */
    private static final class Bindings {

        private final Map<BindingId, Integer> numbered = new HashMap<>();

        int number(BindingId binding) {
            return numbered.computeIfAbsent(binding, it -> numbered.size());
        }

        int of(BindingId binding, String name) {
            Integer number = numbered.get(binding);
            if (number == null) {
                throw new NotLowered("a read of a binding this document does not carry: " + name);
            }
            return number;
        }
    }

    private static String core(Core node, Bindings bindings) {
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
    private static String letIn(Core.LetIn it, Bindings bindings) {
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
    private static String call(Core.Call it, Bindings bindings) {
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
                case Core.Reaches.ABehavior held ->
                        "\"reaches\":\"behavior\",\"declared\":" + quoted(reached(held.declaration()));
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
    private static String match(Core.Match it, Bindings bindings) {
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
    private static String selects(ResolvedCase selected) {
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

    private static String type(Type type) {
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
