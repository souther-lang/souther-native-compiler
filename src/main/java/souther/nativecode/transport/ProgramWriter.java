package souther.nativecode.transport;

import souther.compiler.core.Core;
import souther.compiler.program.CheckedBehavior;
import souther.compiler.program.CheckedImplementation;
import souther.compiler.program.CheckedModule;
import souther.compiler.program.CheckedProgram;
import souther.compiler.types.BindingId;
import souther.compiler.types.Type;
import souther.nativecode.NotLowered;

import java.util.HashMap;
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
        StringJoiner modules = new StringJoiner(",", "[", "]");
        for (CheckedModule module : program.modules()) {
            modules.add(module(module));
        }
        return "{\"transport\":" + TRANSPORT_VERSION + ",\"modules\":" + modules + "}";
    }

    private static String module(CheckedModule module) {
        StringJoiner behaviors = new StringJoiner(",", "[", "]");
        for (CheckedBehavior behavior : module.behaviors()) {
            behaviors.add(behavior(behavior));
        }
        return "{\"name\":" + quoted(module.name()) + ",\"behaviors\":" + behaviors + "}";
    }

    private static String behavior(CheckedBehavior behavior) {
        if (!(behavior.implementation() instanceof CheckedImplementation.Body written)) {
            throw new NotLowered("a behavior implemented other than by a body: " + behavior.name());
        }
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

        void number(BindingId binding) {
            numbered.put(binding, numbered.size());
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
            case Core.Binary it -> "{\"core\":\"binary\",\"op\":" + quoted(it.op().name())
                    + ",\"left\":" + core(it.left(), bindings)
                    + ",\"right\":" + core(it.right(), bindings)
                    + ",\"type\":" + type(it.type()) + "}";

            case Core.Decimal it -> throw notYet("a decimal literal", it);
            case Core.Str it -> throw notYet("a string literal", it);
            case Core.Bool it -> throw notYet("a boolean literal", it);
            case Core.Temporal it -> throw notYet("a temporal literal", it);
            case Core.UnitValue it -> throw notYet("a unit value", it);
            case Core.MaterialisedValue it -> throw notYet("a value read from its module", it);
            case Core.Neg it -> throw notYet("a negation", it);
            case Core.FieldAccess it -> throw notYet("a field access", it);
            case Core.Call it -> throw notYet("a call", it);
            case Core.PreservedCall it -> throw notYet("a call kept for what it says", it);
            case Core.Apply it -> throw notYet("an application of a function value", it);
            case Core.If it -> throw notYet("a condition", it);
            case Core.IfConstructed it -> throw notYet("an attempted construction", it);
            case Core.LetIn it -> throw notYet("a binding", it);
            case Core.Block it -> throw notYet("a function value", it);
            case Core.ListLit it -> throw notYet("a list", it);
            case Core.OptionSome it -> throw notYet("an option holding a value", it);
            case Core.OptionNone it -> throw notYet("an option holding nothing", it);
            case Core.Tuple it -> throw notYet("a tuple", it);
            case Core.TupleGet it -> throw notYet("a member of a tuple", it);
            case Core.Construct it -> throw notYet("a construction", it);
            case Core.Match it -> throw notYet("a match", it);
            case Core.Unreachable it -> throw notYet("an unreachable", it);
        };
    }

    private static NotLowered notYet(String what, Core node) {
        return new NotLowered(what + " at " + node.pos());
    }

    private static String type(Type type) {
        return switch (type) {
            case Type.Prim it when it == Type.Prim.INT -> "{\"prim\":\"INT\"}";

            case Type.Prim it -> throw notYet("the type " + it);
            case Type.Nothing it -> throw notYet("the type " + it);
            case Type.Never it -> throw notYet("the type " + it);
            case Type.Erroneous it -> throw notYet("the type " + it);
            case Type.Var it -> throw notYet("a type variable");
            case Type.MetaVar it -> throw notYet("a type this compiler left open");
            case Type.Ref it -> throw notYet("a declared type");
            case Type.ListOf it -> throw notYet("a list type");
            case Type.MapOf it -> throw notYet("a map type");
            case Type.SetOf it -> throw notYet("a set type");
            case Type.OptionOf it -> throw notYet("an option type");
            case Type.Union it -> throw notYet("a union type");
            case Type.FnOf it -> throw notYet("a function type");
            case Type.TupleOf it -> throw notYet("a tuple type");
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
