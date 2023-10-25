# Checker

Checker converts unfinalized types to finalized types.
The reason this is done here is because the parser can't wait for other
files to parse or a deadlock will occur. Checker is free of this restriction, and will link
types without deadlocking.

Checker also has the secondary job of degenericing any generics and flattening and generic types.
A lot of the code to degeneric types is implemented in that type.