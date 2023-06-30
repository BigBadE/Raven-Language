# References (or the lack thereof)

In Raven, there are no references. This is why the language resembles (and runs in) garbage-collected languages so well.

So how does this work behind the scenes, and how does this effect the user?

# Dual memory management

Raven has two memory management systems: The lifetime memory manager and the "other" memory manager.

The lifetime memory manager uses the lifetime memory management system popularized by Rust, dropping variables when they
go out of scope.

Sometimes, the lifetime memory manager can't free memory, because it has a reference to it outside the scope of the function.

Here's an example in Rust:
```rust
struct Example {
    inner: &Inner,
}

struct Inner {
    id: u32
}

fn main() {
    let inner = Inner { id: 1 };
    let mut example = Example { inner: &inner };
    other_function(&mut example);
    //LIFETIME ISSUE: other_inner is dropped in other_function(), so this fails.
    println!("{}", example.inner.id);
}

fn other_function(example: &mut Example) {
    let other_inner = Inner { id: 2};
    example.inner = other_inner;
}
```

This has a few solutions:
- Disallow references with longer lifespans
- Disallow non-owning references
- Force the user to manually manager those references
- Garbage collect those references
- Reference count the object, dropping it only when there are no references

Each of these solutions have their own languages implementing them, each with their own set of trade-offs.

So how does Raven deal with these, and what does this mean for references?

The first big difference is removing references from being a user's concern, because every function parameter is a reference by default
(Except when the verifier deems that a reference is safe to be directly passed).
The compiler statically analyzes the code to determine lifetimes, changing nothing if a function parameter fits the owner's lifetime,
if not, the compiler falls back on the secondary memory manager, the "other" memory manager.

This allows for the use of slower or more difficult methods on a small subset of the variables used by the program.