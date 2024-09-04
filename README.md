# RelRc

Reference counted pointers that may depend on other pointers. This crate replicates the behaviour of `std::rc::Rc` and `std::rc::Weak` but allows
references to link to other references. A [`RelRc`] object (the equivalent
of `Rc`) will stay alive for as long as there are references either to it or
to one of its descendants.

### Can't I just keep a list of `Rc`s within my `Rc`?
Yes---and that is what this crate does under the hood. This crate however
keeps track of additional data for you, thus providing additional functionalities:
 - dependencies can be traversed backward: [`RelRc::all_children(v)`] will
   return all objects (children) that depend on `v` (parent).
 - data can be stored on the dependency edges themselves.
 - the resulting directed acyclic dependency graph is exposed with [`RelRcGraph`]
   and can be traversed using [`petgraph`] (make sure to activate the
   `petgraph` feature).

This crate can also be viewed as a directed acyclic graph (DAG) implementation,
in which nodes are automatically removed when they and their descendants go out
of scope.
This is a data structure that arises naturally in use cases where new objects
that are created can be viewed as descendants of previously created objects.
Examples include commits in a git history, Merkle trees...

### Node immutability
By design and just like [`Rc`], a [`RelRc`] and its parents are immutable once created.
New [`RelRc`] can be added as descendants of the node, but the node cannot
be modified once it has been created and will exist until all references to
it are dropped.

This ensures that no cycles can be created, removing the need for cyclicity checks and guaranteeing the absence of memory leaks.
If node or edge values need to be mutable, consider using `RefCell`s.

### Example
```rust
use relrc::RelRc;

// Create a Rc pointer (equivalent to `Rc::new`)
let root = RelRc::new("root");

// Now child pointers can be created. Edge value: ()
let child = RelRc::with_parents("child", [(root.clone(), ())]);

// Create a grandchild pointer
let grandchild = RelRc::with_parents("grandchild", [(child.clone(), ())]);

// Obtain a second reference to the child pointer
let child_clone = child.clone();

assert_eq!(root.n_outgoing(), 1, "root has 1 outgoing edge");
assert_eq!(child.n_outgoing(), 1, "child has 1 outgoing edge");

// Drop the original child node
drop(child);

// The second child pointer still exists: outgoing edges remain the same
assert_eq!(root.n_outgoing(), 1, "root still has 1 outgoing edge");
assert_eq!(child_clone.n_outgoing(), 1, "child_clone still has 1 outgoing edge");

// Drop the second child pointer
drop(child_clone);

// Still no change at the root: grandchild node is still a descendant
assert_eq!(root.n_outgoing(), 1, "root still has 1 outgoing edge");

// Finally, dropping the grandchild leaves only the root node
drop(grandchild);
assert_eq!(root.n_outgoing(), 0, "root now has 0 outgoing edges");
```