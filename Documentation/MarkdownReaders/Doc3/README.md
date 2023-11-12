# Getting Started - Types

Types are the backbone of Raven. Every single variable has a type, so types will be covered to help understand variables.

Raven comes with a few built in types, but it's most important to know the "primtives", basic types that all other types are built off of. 
They are the following:

- Unsigned Integers:
Unsigned integers are an integer (whole number) with no sign (positive). There are 4 of them: ``u8``, ``u16``, ``u32``, and ``u64``.
The u stands for unsigned, and the number after is the amount of bits the number has. To understand what the number of bits mean,
it's worth reviewing binary. Keep in mind, if you try to go below 0, unsigned integers will underflow to the maximum number.
Likewise, if you go over the maximum number, the number will overflow to 0.
- Signed Integers:
Signed integers have less range than unsigned integers, but they can be negative. There are also 4 of them: ``i8``, ``i16``, ``i32``, and ``i64``.
The numbers mean the same thing, and i stands for signed.
- Booleans:
There is only one boolean type, ``bool``, which can be the value ``true`` or ``false``.
- Floats:
There are two float types, ``f32`` or ``f64``. Floats, unlike integers, can have decimals. Floats also have a gigantic range,
but it's important to know the drawbacks of floats: Floats are imprecise. Because they're restricted to 32 or 64 bits, they can't
represent every single number. Floats should never be directly compared because they may not precisely be the expected value.
- Strings:
Strings are unique because they can have a variables size. There is only one type, ``str``, but it can be one letter ("a")
or a full sentence ("Hello World!", as seen earlier). That's why a ``str`` isn't mutable. Any operation you do on a ``str``
actually creates a new type.

Now that you've learned the basics, lets move on to actually using those types:

# Variables

Variables are a way to store data for later use. Let's write simple code to square a number:

```
fn main() {
    let squaring = 2;
    let squared = squaring * squaring;
}
```

In this example the line ``let squaring = 2;`` sets the variable ``squaring`` equal to the ``u64`` ``2``.
We can use this variable later, for example when we do ```let squared = squaring * squaring;``` which takes the ```squaring```
variable and squares it by multiplying it by itself, then assigning it to ``squared``.

So, reviewing what has been covered so far:
- Every variable has a name and a type
- Numbers are either unsigned, signed, or floats
- Variables can be used like numbers with their name

Now, lets move onto something a little different. [Chapter 3: Control Flow](3_control_flow.md)