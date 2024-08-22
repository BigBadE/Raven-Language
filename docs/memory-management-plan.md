# Memory management

Raven plans to have a dual system, using borrow checking and generational references.

The idea is that the compiler can seamlessly switch between the two systems, depending on the needs of the program.
This is instituted through a few core ideas:

# Ownership

Ownership is when a type is owned by one thing, such as a structure or function.

For example:

```rust
struct Owner {
    owned_value: String
}
```

This applies to functions as well:

```rust
fn first() {
    let owner = new
    Owner {
        owned_value: "Hello, world!"
    };
    second(owner);
    // Errors, because owner is owned by second now!
    println!(owner.owned_value);
}

fn second(owner: Owner) {
    // Doesn't matter
}
```

This is called a "linear type", it's created once and used once. Loans are one way to get around this restriction, which
are denoted with the & symbol. Loans allow "loaning" parts of the object to other functions, which the compiler
auto-detects:

```rust
fn first() {
    let owner = new
    Owner {
        owned_value: "Hello, world!"
    };
    second(&owner);
    // No error, because owner is owned by first still, second just got a loan for it!
    println!(owner.owned_value);
}

// The Owner object is loaned
fn second(owner: &mut Owner) {
    // Doesn't matter, as long as the loan is dropped by the end of the function.
    // This function can use any field of owner, mutably
}
```

Now, this does come with two caveats:

- A loan can not exist after the original object is dropped (objects drop when their parents drop)
- Only one mutable loan can be made to a piece of data, there can't be a mutable and immutable loan, or multiple mutable
  loans.

Now, if you use Rust, you may notice that a loan is to specific fields instead of to the entire object, like a borrow.
This is a unique difference between Rust and Raven, allowing for code like this:

```rust
struct MyStruct {
    objects: Vec<String>,
    other_data: u64
}

impl MyStruct {
    fn requires_mut(& { mut other_data } self ) {}
}

fn example(my_struct: MyStruct) {
    for element in &my_struct.objects {
        // Allowed, as long as requires_mut doesn't require a loan on my_struct.objects
        // Rust wouldn't allow this, since my_struct would be borrowed immutably by loop
        my_struct.requires_mut();
    }
}
```

This also works inside structs:

```rust
struct ExampleMap {
    data: Vec<String>,
    map: HashMap<String, &{ data } String>
}
```

This would require anything adding a value to map be a reference to the data field, so Raven can copy/move it correctly.

# Generational References

Generational references are a way to reference a value without owning it. This is similar to a pointer, with one notable
difference:
Generational references are completely safe. The method is pretty simple, all references have a "generation" number,
and the data they point to has a generation number at the start. If they don't match, then it must have been dropped.

There's a chance it just happens to match, but it's a 1 / 2^64 chance, which is low enough to be
statistically impossible.

This is denoted with the * symbol:

```rust
struct LinkedListNode {
    next: Option<* LinkedListNode>,
    data: u64
}

struct LinkedList {
    head: Option<* LinkedListNode>
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