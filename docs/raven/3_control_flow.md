# Control Flow

Control Flow is how we use different statements to control the flow of the program. What does that mean? Well, lets look at an example:
What if we wanted to run some code only if a number is less than 5? Let's try it:
```
import stdio;

fn main() {
    let value = 7;
    if value < 5 {
        printf("Yep!");
    } else {
        printf("Nope!");
    }
}
```

Let's run the code with various values:
```
value | output
------+-------
  7   | Nope!
  6   | Nope!
  5   | Nope!
  4   | Yep!
```

So, if the value is less than 5, only the code in the ``if`` statement runs. If the value is greater than or equal to 5, than the code
in the ``else`` statement runs. We can expand this with the ``else if`` statement:

```
import stdio;

fn main() {
    let value = 7;
    if value < 5 {
        printf("Yep!");
    } else if value == 5 {
        printf("Five!");
    } else {
        printf("Nope!");
    }
}
```


Let's run the code with the same values:
```
value | output
------+-------
  7   | Nope!
  6   | Nope!
  5   | Five!
  4   | Yep!
```

Following alone with the code, we can label each statement:
(This code will NOT compile, it is purely for comprehension)

```
import stdio;

fn main() {
    let value = 7;
    1: if value < 5 {
        printf("Yep!");
    2: } else if value == 5 {
        printf("Five!");
    3: } else {
        printf("Nope!");
    }
}
```

The logic is as follows:

1 is checked. IF value is less than 5, THEN Yep! is printed and nothing else happens.
2 is checked IF 1 failed. IF value equals 5, THEN Five! is printed and nothing else happens.
IF 1 and 2 failed, THEN Nope! is printed and nothing else happens.

If the logic doesn't make sense, feel free to mess around with the code on your own.

