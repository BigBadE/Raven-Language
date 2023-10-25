# Traits

Traits are things that can be applied to structures. They basically describe that capabilities of the structure, for example:

```
trait ToString {
    fn to_string(self) -> str;
}
```

This trait says that any structure that implements it has the ``to_string`` method. Let's look at our structure example:

```
impl ToString for MyStruct {
    fn to_string(self) -> str {
        return self.name;
    }
}

fn main() {
    let my_structure = new MyStructure {
        name: "Test!",
        value: 2,
    };
    printf(my_structure.to_string());
}
```

This will print ``Test!`` because it calls the ``to_string`` function of the ``ToString`` trait.
The impl part will tell the compiler that ``MyStruct`` is the trait ``ToString``, and it can have ``to_string`` called on it.

Because traits describe a capability of the struct, the struct itself isn't needed to call trait methods:

```
fn print_to_string(string: ToString) {
    printf(string.to_string());
}
```

This will print whatever the ``ToString`` function ``to_string`` returns. This can be called with ``MyStruct`` or
anything else that implements ``ToString``.

Now, before we go more into depth about traits, it's important to learn about generics. [6 - Generics](6_generics.md)