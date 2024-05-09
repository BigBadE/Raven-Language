# Memory management

Raven plans to have a dual system, using borrow checking with linear types and generational references.

The idea is that the compiler can seamlessly switch between the two systems, depending on the needs of the program.
This is instituted through a few core ideas:

# Ownership

Ownership is when a type is owned by one thing, such as a structure or function.

For example:

```rust
struct Owner {
    owned_value: str
}
```

This has a bunch of advantages:

- It's easy to reason when the type gets dropped, allowed zero-overhead memory management
- Custom drop logic can be implemented, such as requiring a function call before dropping

Some structs can require ownership, such as a future that needs to be awaited.
In most cases, however, struct ownership is not required. It's instead automatically determined by the compiler to
reduce
memory
management overhead.
A majority of variables are simply owned and dropped when they go out of scope, however there needs to be some form of
"backup" when the compiler can't reason when the variable should be dropped.

# Generational References

Generational references are a way to reference a value without owning it. This is similar to a pointer, with one notable
difference:
Generational references are completely safe. The method is pretty simple, all references have a "generation" number,
and the data they point to has a generation number at the start. If they don't match, then it must have been dropped.

Of course, there's a chance it just happens to match, but it's a 1 / 2^64 chance, which is low enough to be
statistically impossible.

This is denoted with the & symbol:

```rust
struct LinkedListNode {
    next: &LinkedListNode,
    data: u64
}
```

These are technically owned, allowing them to have custom drop requirements. For example:

```rust
struct LinkedList {
    head: Option<&LinkedListNode>
}

impl Drop for LinkedList {
    fn drop(&self) {
        let mut current = self.head;
        while let Some(found) = current {
            current = found.next;
            found.drop(true);
        }
    }
}
```

Of course, false can be passed to not drop the base value. This can allow memory leaks, but those don't violate memory
safety.

The issue with these references is twofold:

- There's overhead to check the generation number
- Methods need to be rewritten to take references

There's a few ways to fix these issues, which all stem from "pure" functions which have no side effects.

For example:

```rust
fn print_list(list: &Vec<Foo>) {
    for item in list {
        // Generation check
        printf(item);
    }
}
```

Since the function doesn't ever modify the list, it can be rewritten as:

```rust
fn print_list(list: Vec<Foor>) {
    for item in list {
        // No generation check
        printf(item);
    }
}
```

Then it can be called with a generational reference or a normal reference, as long as the generational reference checks
if
the value is still alive before calling. This is inspired by Vale's "pure" functions, but it doesn't require the
keyword.

