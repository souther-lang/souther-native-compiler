package souther.bindings;

import org.jspecify.annotations.Nullable;
import souther.bindings.Manifest.FunctionCrossing;
import souther.bindings.Manifest.ListCrossing;
import souther.bindings.Manifest.Module;
import souther.bindings.Manifest.Shape;
import souther.bindings.Manifest.Type;
import souther.bindings.Manifest.Way;
import souther.bindings.Manifest.Word;

import java.util.ArrayList;
import java.util.List;

/**
 * A value of a type crossing in a shape, the two taken apart together once: what every generator
 * would otherwise walk for itself before it could say what its language holds.
 *
 * <p>The manifest says a type beside the shape it crosses in ({@link Shape}), and a generator
 * needs both at every level: which primitive a word is, which tuple a product is, what a list or a
 * function value of this shape is reached through. {@link #of} pairs them, checks that they agree
 * (an option with an option, a tuple with a product of as many members, a list, a set or a map
 * with a list of what it holds, a function with a function of as many parameters), and finds what
 * the module says each list and function value is reached through. What is left to a generator is
 * what its language makes of each part, and whether it has a way to hold it at all.
 *
 * <p>A leaf keeps the pair of its type and its word rather than a rule of which word a primitive
 * crosses as: which word a type crosses in is the driver's to choose, and a generator asks of the
 * pair whether its language can hold it.
 */
public sealed interface ValueCrossing {

    /** The type the value is, as the model says it. */
    Type type();

    /** The shape the value crosses in, as the manifest says it. */
    Shape shape();

    /** A value of a primitive, crossing as one word. */
    record Primitive(Type.Primitive type, Word word) implements ValueCrossing {

        @Override
        public Shape shape() {
            return new Shape.Leaf(word);
        }
    }

    /** A value of a declared type, crossing as the address of it ({@link Word#VALUE}). */
    record Handle(Type.Declared type) implements ValueCrossing {

        @Override
        public Shape shape() {
            return new Shape.Leaf(Word.VALUE);
        }
    }

    /**
     * A value of a union no declaration names, crossing as the address of it ({@link Word#VALUE}).
     * A host told nothing of which case it is has no way to hold one, which is each generator's to
     * say where it is handed one.
     */
    record Union(Type.Union type) implements ValueCrossing {

        @Override
        public Shape shape() {
            return new Shape.Leaf(Word.VALUE);
        }
    }

    /** An optional: a presence, then what it holds where it holds something. */
    record Optional(Type.Option type, ValueCrossing of) implements ValueCrossing {

        @Override
        public Shape shape() {
            return new Shape.Option(of.shape());
        }
    }

    /** A tuple, as its members one after another. */
    record Tuple(Type.Tuple type, List<ValueCrossing> members) implements ValueCrossing {

        public Tuple {
            members = List.copyOf(members);
        }

        @Override
        public Shape shape() {
            return new Shape.Product(members.stream().map(ValueCrossing::shape).toList());
        }
    }

    /**
     * A list, a set or a map, as the list of what it holds ({@link Type#listed}), built and read
     * through what the module says for lists of the element's shape.
     */
    record Listed(Type type, ValueCrossing element, ListCrossing crossing)
            implements ValueCrossing {

        @Override
        public Shape shape() {
            return new Shape.ListOf(element.shape());
        }

        /**
         * Whether the module says a way for one to cross {@code way}: built where a host hands one
         * over, read where it is handed one.
         */
        public boolean crosses(Way way) {
            return way == Way.GIVEN ? crossing.construct() != null : crossing.read() != null;
        }
    }

    /**
     * A function value: what it takes and answers, each crossing as the signature says, and what
     * the module says a function value of that signature is called and made through.
     */
    record FunctionValue(Type.Function type, Shape.FunctionOf shape, List<ValueCrossing> takes,
                         ValueCrossing answers, FunctionCrossing crossing)
            implements ValueCrossing {

        public FunctionValue {
            takes = List.copyOf(takes);
        }

        /**
         * Whether the module says a way for one to cross {@code way}: one a host is handed is the
         * library's, called through {@code call}; one it hands over may be its own, made through
         * {@code make}. What the function takes crosses the other way from the value, and what it
         * answers the same way.
         */
        public boolean crosses(Way way) {
            return way == Way.HANDED ? crossing.call() != null : crossing.make() != null;
        }
    }

    /**
     * A value of {@code type} crossing in {@code shape} in a function of {@code module}'s, or null
     * where the two do not agree: a shape of one kind beside a type of another, a product beside a
     * tuple of another count, a leaf beside a type no word is, or a declared type or a union
     * beside any word but {@link Word#VALUE}.
     */
    static @Nullable ValueCrossing of(Module module, Type type, Shape shape) {
        return switch (shape) {
            case Shape.Leaf leaf -> switch (type) {
                case Type.Primitive it -> new Primitive(it, leaf.word());
                case Type.Declared it -> leaf.word() == Word.VALUE ? new Handle(it) : null;
                case Type.Union it -> leaf.word() == Word.VALUE ? new Union(it) : null;
                default -> null;
            };
            case Shape.Option option -> type instanceof Type.Option it
                    && of(module, it.of(), option.of()) instanceof ValueCrossing of
                    ? new Optional(it, of) : null;
            case Shape.Product product -> type instanceof Type.Tuple it
                    && all(module, it.of(), product.of()) instanceof List<ValueCrossing> members
                    ? new Tuple(it, members) : null;
            case Shape.ListOf list -> Type.listed(type) instanceof Type listed
                    && of(module, listed, list.element()) instanceof ValueCrossing element
                    ? new Listed(type, element, module.listOf(list.element())) : null;
            case Shape.FunctionOf function -> type instanceof Type.Function it
                    && all(module, it.takes(), function.signature().takes())
                    instanceof List<ValueCrossing> takes
                    && of(module, it.answers(), function.signature().answers())
                    instanceof ValueCrossing answers
                    ? new FunctionValue(it, function, takes, answers,
                            module.functionOf(function.signature())) : null;
        };
    }

    /**
     * Each of {@code types} crossing in the shape beside it, or null where the two say different
     * counts or any pair does not agree.
     */
    static @Nullable List<ValueCrossing> all(Module module, List<Type> types, List<Shape> shapes) {
        if (types.size() != shapes.size()) {
            return null;
        }
        List<ValueCrossing> made = new ArrayList<>();
        for (int at = 0; at < types.size(); at++) {
            ValueCrossing it = of(module, types.get(at), shapes.get(at));
            if (it == null) {
                return null;
            }
            made.add(it);
        }
        return made;
    }
}
