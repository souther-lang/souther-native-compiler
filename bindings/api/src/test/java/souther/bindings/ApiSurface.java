package souther.bindings;

import java.lang.annotation.Annotation;
import java.lang.reflect.AnnotatedArrayType;
import java.lang.reflect.AnnotatedElement;
import java.lang.reflect.AnnotatedParameterizedType;
import java.lang.reflect.AnnotatedType;
import java.lang.reflect.AnnotatedTypeVariable;
import java.lang.reflect.AnnotatedWildcardType;
import java.lang.reflect.Constructor;
import java.lang.reflect.Executable;
import java.lang.reflect.Field;
import java.lang.reflect.Method;
import java.lang.reflect.Modifier;
import java.lang.reflect.RecordComponent;
import java.lang.reflect.TypeVariable;
import java.nio.file.Files;
import java.nio.file.Path;
import java.util.ArrayList;
import java.util.List;
import java.util.Set;
import java.util.TreeSet;
import java.util.stream.Collectors;
import java.util.stream.Stream;

/**
 * The surface of this API as a generator compiled against it depends on, one line for each fact:
 * each public type and what it extends, each public or protected member with its signature, and, for
 * what a generator switches over or implements, one line holding the whole of it: the cases of a
 * sealed type, the constants of an enum, the components of a record, and the abstract methods of a
 * type nothing seals. A case, a constant, a component or an abstract method added is then a line
 * changed rather than a line added, and changes what the major promised ({@link BindingApi}).
 *
 * <p>Types are written as annotated types, so a JSpecify nullness annotation declared on one is part
 * of its line wherever it stands, a type argument among them.
 */
final class ApiSurface {

    private static final String PACKAGE = "souther.bindings";
    private static final String JSPECIFY = "org.jspecify.annotations.";

    private ApiSurface() {
    }

    /** The surface of the public types the API's classes declare, {@code anchor} among them. */
    static Set<String> of(Class<?> anchor) throws Exception {
        Set<String> lines = new TreeSet<>();
        for (Class<?> type : publicTypes(anchor)) {
            describe(type, lines);
        }
        return lines;
    }

    private static void describe(Class<?> type, Set<String> lines) {
        String name = name(type);
        StringBuilder declared = new StringBuilder("type ").append(name).append(' ')
                .append(kind(type)).append(typeParameters(type.getTypeParameters()));
        if (type.getAnnotatedSuperclass() != null && !type.isRecord() && !type.isEnum()
                && type.getSuperclass() != Object.class) {
            declared.append(" extends ").append(render(type.getAnnotatedSuperclass()));
        }
        List<String> interfaces = Stream.of(type.getAnnotatedInterfaces()).map(ApiSurface::render)
                .toList();
        if (!interfaces.isEmpty()) {
            declared.append(type.isInterface() ? " extends " : " implements ")
                    .append(String.join(", ", interfaces));
        }
        declared.append(annotations(type));
        lines.add(declared.toString());

        if (type.isSealed()) {
            lines.add("sealed " + name + " = " + Stream.of(type.getPermittedSubclasses())
                    .map(ApiSurface::name).sorted().collect(Collectors.joining(",")));
        } else if (!Modifier.isFinal(type.getModifiers())) {
            lines.add("abstract " + name + " = " + Stream.of(type.getMethods())
                    .filter(it -> Modifier.isAbstract(it.getModifiers()))
                    .map(ApiSurface::signature).sorted().collect(Collectors.joining(",")));
        }
        if (type.isEnum()) {
            lines.add("enum " + name + " = " + Stream.of(type.getEnumConstants())
                    .map(it -> ((Enum<?>) it).name()).collect(Collectors.joining(",")));
        }
        if (type.isRecord()) {
            lines.add("record " + name + " = (" + Stream.of(type.getRecordComponents())
                    .map(it -> render(it.getAnnotatedType()) + " " + it.getName())
                    .collect(Collectors.joining(", ")) + ")");
        }
        for (Method method : type.getDeclaredMethods()) {
            if (visible(method.getModifiers()) && !method.isSynthetic() && !method.isBridge()) {
                lines.add("method " + modifiers(method.getModifiers()) + name + "."
                        + signature(method));
            }
        }
        for (Constructor<?> constructor : type.getDeclaredConstructors()) {
            if (visible(constructor.getModifiers()) && !constructor.isSynthetic()) {
                lines.add("constructor " + name + "(" + parameters(constructor) + ")"
                        + exceptions(constructor));
            }
        }
        for (Field field : type.getDeclaredFields()) {
            if (visible(field.getModifiers()) && !field.isSynthetic()) {
                lines.add("field " + modifiers(field.getModifiers()) + name + "."
                        + field.getName() + " : " + render(field.getAnnotatedType())
                        + constant(field));
            }
        }
    }

    private static String signature(Method method) {
        return typeParameters(method.getTypeParameters()) + method.getName() + "("
                + parameters(method) + ") -> " + render(method.getAnnotatedReturnType())
                + exceptions(method);
    }

    private static String parameters(Executable executable) {
        return Stream.of(executable.getAnnotatedParameterTypes()).map(ApiSurface::render)
                .collect(Collectors.joining(", "));
    }

    private static String exceptions(Executable executable) {
        List<String> thrown = Stream.of(executable.getAnnotatedExceptionTypes())
                .map(ApiSurface::render).sorted().toList();
        return thrown.isEmpty() ? "" : " throws " + String.join(", ", thrown);
    }

    private static String typeParameters(TypeVariable<?>[] variables) {
        if (variables.length == 0) {
            return "";
        }
        return "<" + Stream.of(variables).map(it -> it.getName() + " extends "
                        + Stream.of(it.getAnnotatedBounds()).map(ApiSurface::render)
                        .collect(Collectors.joining(" & ")))
                .collect(Collectors.joining(", ")) + "> ";
    }

    /** What a static final constant is, since a generator compiled against it holds a copy of it. */
    private static String constant(Field field) {
        int modifiers = field.getModifiers();
        if (!Modifier.isStatic(modifiers) || !Modifier.isFinal(modifiers)
                || !(field.getType().isPrimitive() || field.getType() == String.class)) {
            return "";
        }
        try {
            return " = " + field.get(null);
        } catch (IllegalAccessException e) {
            throw new IllegalStateException(e);
        }
    }

    private static String render(AnnotatedType type) {
        String annotated = annotations(type);
        String prefix = annotated.isEmpty() ? "" : annotated.strip() + " ";
        return switch (type) {
            case AnnotatedArrayType array -> render(array.getAnnotatedGenericComponentType())
                    + (annotated.isEmpty() ? "" : " " + annotated.strip()) + "[]";
            case AnnotatedParameterizedType parameterized -> prefix
                    + name((Class<?>) ((java.lang.reflect.ParameterizedType) parameterized.getType())
                    .getRawType()) + "<" + Stream.of(parameterized.getAnnotatedActualTypeArguments())
                    .map(ApiSurface::render).collect(Collectors.joining(", ")) + ">";
            case AnnotatedWildcardType wildcard -> prefix + "?"
                    + bounds(" extends ", wildcard.getAnnotatedUpperBounds(), true)
                    + bounds(" super ", wildcard.getAnnotatedLowerBounds(), false);
            case AnnotatedTypeVariable variable -> prefix
                    + ((TypeVariable<?>) variable.getType()).getName();
            default -> prefix + (type.getType() instanceof Class<?> it ? name(it)
                    : type.getType().getTypeName());
        };
    }

    private static String bounds(String word, AnnotatedType[] bounds, boolean upper) {
        List<String> written = Stream.of(bounds)
                .filter(it -> !(upper && it.getType() == Object.class
                        && annotations(it).isEmpty()))
                .map(ApiSurface::render).toList();
        return written.isEmpty() ? "" : word + String.join(" & ", written);
    }

    /** The JSpecify annotations on {@code element}, each as {@code @Name}, and no others. */
    private static String annotations(AnnotatedElement element) {
        StringBuilder written = new StringBuilder();
        for (Annotation annotation : element.getDeclaredAnnotations()) {
            String type = annotation.annotationType().getName();
            if (type.startsWith(JSPECIFY)) {
                written.append(" @").append(type.substring(JSPECIFY.length()));
            }
        }
        return written.toString();
    }

    private static String kind(Class<?> type) {
        String sealing = type.isSealed() ? "sealed " : "";
        if (type.isAnnotation()) {
            return "annotation";
        }
        if (type.isInterface()) {
            return sealing + "interface";
        }
        if (type.isRecord()) {
            return "record";
        }
        if (type.isEnum()) {
            return "enum";
        }
        return sealing + (Modifier.isFinal(type.getModifiers()) ? "final "
                : Modifier.isAbstract(type.getModifiers()) ? "abstract " : "") + "class";
    }

    private static String modifiers(int modifiers) {
        String written = Modifier.toString(modifiers & (Modifier.STATIC | Modifier.FINAL
                | Modifier.ABSTRACT | Modifier.PROTECTED));
        return written.isEmpty() ? "" : written + " ";
    }

    private static boolean visible(int modifiers) {
        return Modifier.isPublic(modifiers) || Modifier.isProtected(modifiers);
    }

    /** A type of this API by its name within the package, and any other by its full name. */
    private static String name(Class<?> type) {
        if (type.isArray()) {
            return name(type.getComponentType()) + "[]";
        }
        String full = type.getName();
        return full.startsWith(PACKAGE + ".") ? full.substring(PACKAGE.length() + 1).replace('$', '.')
                : full.replace('$', '.');
    }

    /** Every public type the classes {@code anchor} was built with declare, nested ones among them. */
    private static List<Class<?>> publicTypes(Class<?> anchor) throws Exception {
        Path classes = Classes.of(anchor);
        List<Class<?>> types = new ArrayList<>();
        try (Stream<Path> files = Files.walk(classes.resolve(PACKAGE.replace('.', '/')))) {
            for (Path file : files.filter(it -> it.toString().endsWith(".class")).sorted().toList()) {
                String binary = Classes.nameOf(classes, file);
                Class<?> type = Class.forName(binary, false, anchor.getClassLoader());
                if (publicAllTheWayOut(type) && !type.isAnonymousClass() && !type.isSynthetic()) {
                    types.add(type);
                }
            }
        }
        return types;
    }

    private static boolean publicAllTheWayOut(Class<?> type) {
        for (Class<?> at = type; at != null; at = at.getEnclosingClass()) {
            if (!Modifier.isPublic(at.getModifiers())) {
                return false;
            }
        }
        return true;
    }
}
