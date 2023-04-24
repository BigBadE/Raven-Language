# Operators

Operators are shortcuts to method calls like ```+```, ```-```, ```/```, etc...

Operators are defined using the "operation" keyword

# Joint Operators

Operators can be annotated with the ```#[join({join_group})]``` annotation.

Joint operators have the following structure:

```fn join_operator<In, Out>(values: [In; 3], lhs: fn(&In, &In) -> Out, rhs: fn(&In, &In) -> Out) -> Out```

For example, if we have the operators for comparison:

```<```, ```>```, ```<=```, ```>=```

We can allow them to be joined together like this:

```1 <= 3 < 5```

By annotating them all with ```#[join(comparison)]```

Then implementing it with:

```fn join_operator<T>(values: [T; 3], lhs: fn(&T, &T) -> bool, rhs: fn(&T, &T) -> bool) -> bool {
    return lhs(&values[0], &values[1]) && rhs(&values[1], &values[2]);
}```