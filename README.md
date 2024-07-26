### RCDag

A directed acyclic graph (DAG) implementation using reference counting.

A node is only part of the DAG for as long as the client keeps a reference
to it or to one of its descendants. This is a data structure that arises
naturally in use cases where new objects that are created can be viewed as
descendants of previously created objects.
Examples include commits in a git history, Merkle trees...

### Node immutability
By design, a node and all incoming edges to it is immutable once created.
New nodes can be added as descendants of the node, but the node cannot
be modified once it has been created and will exist until all references to
it are dropped.

This ensures that no cycles can be created, removing the need for cyclicity checks and guaranteeing the absence of memory leaks.
Another consequence of this design decision is that all DAGs will have a single
source, from which all descending nodes can be reached.
If node or edge values need to be mutable, consider using `RefCell`s.

### Example
```rust
use rc_dag::Node;

// Create the root node
let root = Node::new("root");

// Create a child node
let child = Node::with_incoming("child", [(root.clone(), ())]);

// Create a grandchild node
let grandchild = Node::with_incoming("grandchild", [(child.clone(), ())]);

// Clone the child node
let child_clone = child.clone();

assert_eq!(root.n_outgoing(), 1, "root has 1 outgoing edge");
assert_eq!(child.n_outgoing(), 1, "child has 1 outgoing edge");

// Drop the original child node
drop(child);

// The cloned child node still exists: outgoing edges remain the same
assert_eq!(root.n_outgoing(), 1, "root still has 1 outgoing edge");
assert_eq!(child_clone.n_outgoing(), 1, "child_clone still has 1 outgoing edge");

// Drop the cloned child node
drop(child_clone);

// Still no change at the root: grandchild node is still a descendant
assert_eq!(root.n_outgoing(), 1, "root still has 1 outgoing edge");

// Finally, dropping the grandchild leaves only the root node
drop(grandchild);
assert_eq!(root.n_outgoing(), 0, "root now has 0 outgoing edges");
```