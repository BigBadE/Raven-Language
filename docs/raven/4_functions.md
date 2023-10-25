Let's finally look at the Hello World code again:

```
import stdio;

fn main() {
    printf("Hello World!");
}
```

Now that we've learned types, we know "Hello World!" is a ``str``. So what about printf and main? Well,
they're both functions. Functions allow you to re-use your code easily, and in Raven, all code must be in a function.
In that example, we make one function called ``main``. This function main is what the compiler calls to start your code,
if you named it ``hello_world`` then nothing will happen (in fact, the compiler will error).

After the name, there's two parenthesis. These are the arguments to the function, and the main function must have none.
Of course, some functions have arguments, such as ``printf`` in the example. Here's what that function would look like:
```
fn printf(string: str) {
    // Code here
}
```

This takes in a single argument ``string`` which is a ``str``. We can use this function with ``printf()``, but it must
have a single argument that's a string. ``printf("Test!")`` works, but ``printf()`` or ``printf(1)`` or ``printf("Hello", "World!")`` doesn't.

Functions can also call each other, consider the Fibonacci sequence:

0, 1, 1, 2, 3, 5, 8, 13, ...

You can get the next Fibonacci number by summing the last two Fibonacci numbers. Let's write a function to do this:

```
fn fibonacci(number: u64) {
    if number == 0 {
        return 0;
    } else if number == 1 {
        return 1;
    } else {
        return fibonacci(number-2) + fibonacci(number-1);
    }
}
```

In fact, functions are everywhere in code. Even basic addition like ``1 + 2`` actually calls an ``add`` function under the hood.

Now that you've learned functions, move on to more complex types in [Chapter 5: Structures](5_structures.md).