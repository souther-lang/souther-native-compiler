package souther.bindings;

/**
 * What a generator is given of a library: what the host's language surface is derived from, and
 * what is carried over as the driver wrote it.
 *
 * @param manifest     the library's model, from which a binding's surface is derived, and nothing
 *                     else of the program is reachable
 * @param declarations the ABI the driver settled, carried over and not read
 */
public record BindingInput(Manifest manifest, Declarations declarations) {
}
