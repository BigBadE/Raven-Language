# Structures

If functions are a bundle of code, structures are simply bundles of variables.
Structures are your own custom type which bundles together a bunch of variables for easy use.

Let's look at an example:

```
pub struct MyStructure {
    name: str,
    value: u64,
}
```

This creates a structure called MyStructure with a field ``name`` that's a ``str`` and a field ``value`` that's a ``u64``.

Structs can be created and have their fields accessed:

```
fn main() {
    let my_structure = new MyStructure {
        name: "Test!",
        value: 2,
    };
    printf(my_structure.name);
}
```

This will print ``Test!``.

Structures are types, so they can be function arguments as well:

```
fn my_function(my_structure: MyStructure) {
    printf(my_structure.name);
}
```

Now, lets look at the next step of structures: [5.1 - Traits](5_1_traits.md)