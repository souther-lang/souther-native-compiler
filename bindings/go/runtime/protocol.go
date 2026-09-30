package souther

// Protocol is the number of what this package's public surface is, which every generated package
// is written against and refuses to compile without. The surface of each protocol is recorded in
// protocol/<n>.txt, and a test fails where it is not what the record says: a change to what a
// generated package may call is a new protocol, with its own record, and not an edit of one.
//
// It says nothing of the native library, whose ABI generation [Load] asks it for, and everything of
// what a generated package and this package agree on, which Go compiles.
const Protocol = 3
