package souther

// Option is an optional value of the model: a value, or nothing. It is a type of its own, since a
// pointer cannot tell an optional that holds nothing from one that holds an optional that holds
// nothing.
//
// The zero value holds nothing.
type Option[T any] struct {
	value T
	some  bool
}

// Some is an optional that holds value.
func Some[T any](value T) Option[T] { return Option[T]{value, true} }

// None is an optional that holds nothing.
func None[T any]() Option[T] { return Option[T]{} }

// Get is what it holds, and whether it holds one.
func (o Option[T]) Get() (T, bool) { return o.value, o.some }

// IsSome is whether it holds a value.
func (o Option[T]) IsSome() bool { return o.some }

// OrElse is what it holds, or fallback where it holds nothing.
func (o Option[T]) OrElse(fallback T) T {
	if o.some {
		return o.value
	}
	return fallback
}
