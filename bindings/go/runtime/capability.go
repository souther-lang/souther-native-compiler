package souther

import "unsafe"

// Capability stands for a behavior another requires: a behavior bound in turn, or a host's own
// implementation of one. The library calls through it, and reads what it points at, for as long as
// anything bound to it may be called, so it is laid out in C memory a run holds until it ends.
//
// The runtime that made it is the only one it may be handed to: a call made with one another
// runtime made, or bound to one at any depth, is refused before it is made.
type Capability[B any] struct {
	// at is the address of the capability the library wrote, or nil where nothing requires the
	// behavior and none was made.
	at       unsafe.Pointer
	run      *Run[B]
	requires []Capability[B]
	// array is the address of the capabilities of what the behavior requires, one after another,
	// or nil where it requires nothing.
	array unsafe.Pointer
}

// Bound is a behavior bound to what stands for each behavior it requires, in the order it requires
// them, and its own capability where something may require it: bind is the library's function that
// writes it, handed room for it and the address of the capabilities of what it requires.
//
// What is made is held until r ends. The capabilities bound to are borrowed from the runs they were
// made in, and each is asked whether it is good where a call is made.
func Bound[B any](r *Run[B], requires []Capability[B], bind func(capability, requirements unsafe.Pointer)) Capability[B] {
	r.checkMaking()
	c := Capability[B]{run: r, requires: append([]Capability[B](nil), requires...)}
	if len(requires) > 0 {
		layout := r.lib.layout
		c.array = r.room(uintptr(len(requires)) * layout.Pointer)
		for at, required := range requires {
			*(*unsafe.Pointer)(unsafe.Add(c.array, uintptr(at)*layout.Pointer)) = required.at
		}
	}
	if bind != nil {
		c.at = r.room(r.lib.layout.Capability)
		bind(c.at, c.array)
	}
	return c
}

// Implemented is a host's own implementation of a behavior, made into a capability the library
// calls it through: implement is the library's function making it, handed room for the capability,
// room for what it is read out of, and the userdata the library hands the implementation first,
// which holds dispatch. All of it is held until r ends.
func Implemented[B any](r *Run[B], dispatch any, implement func(capability, hosted, userdata unsafe.Pointer)) Capability[B] {
	r.checkMaking()
	c := Capability[B]{run: r}
	c.at = r.room(r.lib.layout.Capability)
	implement(c.at, r.room(r.lib.layout.Hosted), r.userdata(dispatch))
	return c
}

// Requirements is what a call of the behavior in r is handed first: the address of the capabilities
// of what it requires, or nil where it requires nothing.
//
// It panics with a [*Misuse] where the run the behavior was bound in has ended or is on another
// goroutine, and returns [ErrForeignHandle] where another runtime bound it, or bound anything it
// stands on: the call would run another library's code over this one's arena.
func (c Capability[B]) Requirements(r *Run[B]) (unsafe.Pointer, error) {
	c.check()
	if !c.of(r.lib.rt.identity) {
		return nil, ErrForeignHandle
	}
	return c.array, nil
}

func (c Capability[B]) check() {
	if c.run == nil {
		misuse(ErrNoValue)
	}
	c.run.checkReading()
}

// of is whether this and everything it stands on was made by the runtime with this identity.
func (c Capability[B]) of(identity uintptr) bool {
	if c.run.lib.rt.identity != identity {
		return false
	}
	for _, required := range c.requires {
		required.check()
		if !required.of(identity) {
			return false
		}
	}
	return true
}

// HostFunction is a function of the host's own made into a function value the library calls, made
// in r: implement is the library's function making one of the shape, handed room for what it is
// laid out as and the userdata the library hands the function first, which holds what dispatch
// makes. The room and the userdata are held until r ends.
//
// key says which function of the host's this is: one handed over again in this run is the value it
// was made into the first time, however often the library calls it.
func HostFunction[B any](r *Run[B], key any, dispatch func() any, implement func(room, userdata unsafe.Pointer) unsafe.Pointer) unsafe.Pointer {
	r.checkMaking()
	if made, ok := r.held.functions[key]; ok {
		return made
	}
	// souther_hosted_function: what it is called through, and what it is read out of.
	made := implement(r.room(r.lib.layout.HostedFunction), r.userdata(dispatch()))
	if r.held.functions == nil {
		r.held.functions = make(map[any]unsafe.Pointer)
	}
	r.held.functions[key] = made
	return made
}
