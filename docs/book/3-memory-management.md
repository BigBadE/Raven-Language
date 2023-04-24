# Memory Management

Raven doesn't require the user to use references. Instead,
the compiler infers how memory should be managed based of the code.

This is done through static analysis using references:

# References

Say function A calls function B and passes variable C, and B returns D.

This results in the following ownership:

- A owns C
- D references C

That means, if A is dropped, C is dropped, and D's reference is invalid.

So, the compiler will detect D's reference to C, and swap ownership 
of C to D instead of A, and give A the reference.

To do this, A would always only have a pointer to C, it will just be the only
object to drop C unless something else controls C.

