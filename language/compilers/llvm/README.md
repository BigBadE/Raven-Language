# LLVM

This module hooks into the LLVM library, converting the Syntax 
into LLVM-IR which can be compiled by LLVM. 

The internal folder has the internal functions used by core.
This implements things like math, with the internal keyword.

Code with the #[llvm_internal] attribute is different, and automatically
linked by LLVM itself (but internal makes sure they are valid internal functions).