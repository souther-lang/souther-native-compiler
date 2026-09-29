package souther

// A tuple of the model is one of these, by how many members it has, so that a tuple crossing from
// one package to another is one type. The model has tuples of every size; a binding writes the
// ones up to eight members and none beyond.

// Tuple2 is a tuple of 2 members: V0 is the first.
type Tuple2[A any, B any] struct {
	V0 A
	V1 B
}

// Tuple3 is a tuple of 3 members: V0 is the first.
type Tuple3[A any, B any, C any] struct {
	V0 A
	V1 B
	V2 C
}

// Tuple4 is a tuple of 4 members: V0 is the first.
type Tuple4[A any, B any, C any, D any] struct {
	V0 A
	V1 B
	V2 C
	V3 D
}

// Tuple5 is a tuple of 5 members: V0 is the first.
type Tuple5[A any, B any, C any, D any, E any] struct {
	V0 A
	V1 B
	V2 C
	V3 D
	V4 E
}

// Tuple6 is a tuple of 6 members: V0 is the first.
type Tuple6[A any, B any, C any, D any, E any, F any] struct {
	V0 A
	V1 B
	V2 C
	V3 D
	V4 E
	V5 F
}

// Tuple7 is a tuple of 7 members: V0 is the first.
type Tuple7[A any, B any, C any, D any, E any, F any, G any] struct {
	V0 A
	V1 B
	V2 C
	V3 D
	V4 E
	V5 F
	V6 G
}

// Tuple8 is a tuple of 8 members: V0 is the first.
type Tuple8[A any, B any, C any, D any, E any, F any, G any, H any] struct {
	V0 A
	V1 B
	V2 C
	V3 D
	V4 E
	V5 F
	V6 G
	V7 H
}
