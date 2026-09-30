package souther.bindings;

import souther.bindings.Manifest.FunctionCrossing;
import souther.bindings.Manifest.ListCrossing;
import souther.bindings.Manifest.Shape;
import souther.bindings.Manifest.Type;
import souther.bindings.Manifest.Way;
import souther.bindings.Manifest.Word;

import java.util.List;

/**
 * A value of a type crossing in a shape: the two taken apart together, by the command, before a
 * generator is handed the model.
 *
 * <p>Wherever the model says a value crosses — what a behavior, an injection or a function value
 * takes and answers, a published value, a field, what a constructor takes, a list's element — it
 * says it as one of these, and never as a type beside a shape a generator would pair itself. Each
 * record holds its parts to agreeing where it is made (an option with an option, a tuple with a
 * product of as many members, a list, a set or a map with a list of what it holds, a function with
 * a function of as many parameters), so one that says a type crosses in a shape it cannot is one
 * no one can make. What is left to a generator is what its language makes of each part, and
 * whether it has a way to hold it at all.
 *
 * <p>A leaf keeps the pair of its type and its word rather than a rule of which word a type
 * crosses as: which word a type crosses in is the driver's to choose, and a generator asks of the
 * pair whether its language can hold it.
 */
public sealed interface ValueCrossing {

    /** The type the value is, as the model says it. */
    Type type();

    /** The shape the value crosses in. */
    Shape shape();

    /** A value of a primitive, crossing as one word. */
    record Primitive(Type.Primitive type, Word word) implements ValueCrossing {

        @Override
        public Shape shape() {
            return new Shape.Leaf(word);
        }
    }

    /** A value of a declared type, crossing as one word: the address of it, as the driver has it. */
    record Handle(Type.Declared type, Word word) implements ValueCrossing {

        @Override
        public Shape shape() {
            return new Shape.Leaf(word);
        }
    }

    /**
     * A value of a union no declaration names, crossing as one word. A host told nothing of which
     * case it is has no way to hold one, which is each generator's to say where it is handed one.
     */
    record Union(Type.Union type, Word word) implements ValueCrossing {

        @Override
        public Shape shape() {
            return new Shape.Leaf(word);
        }
    }

    /** An optional: a presence, then what it holds where it holds something. */
    record Optional(Type.Option type, ValueCrossing of) implements ValueCrossing {

        public Optional {
            agree(of.type(), type.of(), "what an optional holds");
        }

        @Override
        public Shape shape() {
            return new Shape.Option(of.shape());
        }
    }

    /** A tuple, as its members one after another. */
    record Tuple(Type.Tuple type, List<ValueCrossing> members) implements ValueCrossing {

        public Tuple {
            members = List.copyOf(members);
            agree(members.stream().map(ValueCrossing::type).toList(), type.of(),
                    "the members of a tuple");
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

        public Listed {
            Type listed = Type.listed(type);
            if (listed == null) {
                throw new IllegalArgumentException(type + " is no list, set or map");
            }
            agree(element.type(), listed, "what a list holds");
            if (!crossing.element().equals(element.shape())) {
                throw new IllegalArgumentException("a list of " + element.shape() + " is built and"
                        + " read through the functions for a list of " + crossing.element());
            }
        }

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
    record FunctionValue(Type.Function type, List<ValueCrossing> takes, ValueCrossing answers,
                         FunctionCrossing crossing) implements ValueCrossing {

        public FunctionValue {
            takes = List.copyOf(takes);
            agree(takes.stream().map(ValueCrossing::type).toList(), type.takes(),
                    "what a function value takes");
            agree(answers.type(), type.answers(), "what a function value answers");
            Manifest.Signature signature = new Manifest.Signature(shapes(takes), answers.shape());
            if (!crossing.signature().equals(signature)) {
                throw new IllegalArgumentException("a function value of " + signature
                        + " is called and made through the functions for one of "
                        + crossing.signature());
            }
        }

        @Override
        public Shape.FunctionOf shape() {
            return new Shape.FunctionOf(new Manifest.Signature(shapes(takes), answers.shape()));
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

    /** The shapes of each of {@code crossings}, in order. */
    static List<Shape> shapes(List<ValueCrossing> crossings) {
        return crossings.stream().map(ValueCrossing::shape).toList();
    }

    /** The types of each of {@code crossings}, in order. */
    static List<Type> types(List<ValueCrossing> crossings) {
        return crossings.stream().map(ValueCrossing::type).toList();
    }

    /** Refuses {@code crossing} as {@code of} unless it is a value of {@code type}. */
    private static void agree(Object crossing, Object type, String of) {
        if (!crossing.equals(type)) {
            throw new IllegalArgumentException(of + " crosses as " + crossing + ", and is " + type);
        }
    }
}
