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

This means that any type can replace T and E as long as it implements ToString/ToNumber.

A more useful example of this is lists:
```
trait List<T> {
    fn get(index: u64) -> T;
    
    fn insert(index: u64, adding: T);
}
```

Lists all have a single generic parameter, which is what the list is of. ``List<u64>`` has T = ``u64``, so it's a list of ``u64``s

This is taken to the extreme with things like the add and assign trait:
```
// T is what we're adding to, E is what's being added.
// Can be read as adding E to T.
// This trait is implemented for every type where E can be added to T automatically.
pub impl<T: Add<E, T>, E> AddAndAssign<E, T> for T {
    fn add_assign(self, other: E) -> C {
        // This add method must exist and accept other because other is E and T is Add<E, T>
        self = self.add(other);
        return self;
    }
}
```