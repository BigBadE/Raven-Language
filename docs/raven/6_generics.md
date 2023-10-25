# Generics

Let's consider the example from the last section, traits:

```
fn print_to_string(string: ToString) {
    printf(string.to_string());
}
```

This section introduces generics, types that don't have a specific type yet. Before we get into more useful examples,
lets rewrite the last one using generics:

```
fn print_to_string<T: ToString>(string: T) {
    printf(string.to_string());
}
```

This declares a generic type T, that is bounded by ToString. This means that ``print_to_string`` can be called with any type,
as long as it is ToString. So what are the benefits of generics? Well, what if we add a second trait named ``ToNumber``, and we want it to be both?

This isn't possible with just traits, but generics allows it:

```
fn print_to_string<T: ToString + ToNumber>(string: T) {
    printf(string.to_string());
}
```

Generics are also possible on structures, so we can rewrite our ``MyStructure`` from earlier to use generics:

```
struct MyStructure<T: ToString, E: ToNumber> {
    name: T,
    value: E
}
```